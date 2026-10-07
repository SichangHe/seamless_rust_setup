//! File-based "pry" point: a program pauses at `pry::session(&mut ctx, dir)`
//! and runs snippet files dropped into `dir/req/` against `ctx`.
//! Design: `docs/interactive.md`.
use std::{
    any::type_name,
    ffi::{CString, c_char, c_int, c_void},
    fs, io,
    io::Write,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
    process::Command,
    thread::sleep,
    time::{Duration, Instant},
};

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn dup(fd: c_int) -> c_int;
    fn dup2(fd: c_int, fd2: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
}
const RTLD_NOW: c_int = 2;
const POLL: Duration = Duration::from_millis(2);

/// Snippet entry points. Expands to `pry_type` (used by the host to refuse a
/// snippet written against the wrong context type) and `pry_main`.
#[macro_export]
macro_rules! snippet {
    (|$ctx:ident: &mut $T:ty| $body:block) => {
        #[unsafe(no_mangle)]
        pub fn pry_type() -> &'static str {
            ::std::any::type_name::<$T>()
        }
        #[unsafe(no_mangle)]
        pub fn pry_main($ctx: &mut $T) $body
    };
}

/// 🧑 "much better interactive use by editing and loading a file or files
/// instead of doing it REPL style and optimized for agents"
/// Block here and serve snippet requests until one named `*.continue` arrives.
/// Request `N.rs` in `dir/req/` produces `dir/res/N.out` (captured stdout and
/// stderr, or rustc diagnostics) and `dir/res/N.status`: `ok`, `compile_error`,
/// `type_mismatch`, `panic`, or `load_error`. Timings go to `N.status`'s
/// second line as `compile_ms run_ms`.
///
/// # Safety
/// All submitted snippets must be trusted and use the host's exact compiler,
/// dependency artifacts and context layout. A matching type name does not
/// establish ABI compatibility. Snippets must return without retaining `ctx`.
pub unsafe fn session<T>(ctx: &mut T, dir: impl AsRef<Path>) {
    let dir = dir.as_ref();
    let (req, res, build) = (dir.join("req"), dir.join("res"), dir.join("build"));
    for d in [&req, &res, &build] {
        fs::create_dir_all(d).expect("pry: cannot create session dir");
    }
    let externs = externs_from_deps();
    let mut generation = 0_u64;
    eprintln!(
        "pry: waiting in {}, ctx: {}",
        dir.display(),
        type_name::<T>()
    );
    loop {
        let Some(path) = next_request(&req) else {
            sleep(POLL);
            continue;
        };
        if path.extension().is_some_and(|e| e == "continue") {
            let _ = fs::remove_file(&path);
            return;
        }
        let stem = path
            .file_stem()
            .expect("file")
            .to_string_lossy()
            .into_owned();
        let library_dir = loop {
            let candidate = build.join(generation.to_string());
            generation += 1;
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("pry: reserve library path: {error}"),
            }
        };
        let so = library_dir.join("snippet.so");
        let (status, out) = eval(ctx, &path, &so, &externs);
        let _ = fs::remove_file(&path);
        write_atomic(&res.join(format!("{stem}.out")), &out);
        write_atomic(&res.join(format!("{stem}.status")), &status);
    }
}

fn next_request(req: &Path) -> Option<PathBuf> {
    let mut names: Vec<PathBuf> = fs::read_dir(req)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "rs" || e == "continue"))
        .collect();
    names.sort();
    names.into_iter().next()
}

fn write_atomic(path: &Path, content: &str) {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content)
        .and_then(|()| fs::rename(&tmp, path))
        .expect("pry: write result");
}

/// Compile, load, type-check, run; returns `(status, output)`.
fn eval<T>(ctx: &mut T, src: &Path, so: &Path, externs: &[String]) -> (String, String) {
    let t0 = Instant::now();
    let mut cmd = Command::new("rustc");
    cmd.args([
        "--edition",
        "2024",
        "--crate-type",
        "cdylib",
        "-C",
        "prefer-dynamic",
    ])
    .args(["-C", "debuginfo=0", "-A", "warnings", "--color", "never"])
    .args(externs)
    .arg("-o")
    .arg(so)
    .arg(src);
    let compiled = match cmd.output() {
        Ok(o) => o,
        Err(e) => return ("compile_error".into(), format!("spawn rustc: {e}")),
    };
    let compile_ms = t0.elapsed().as_millis();
    if !compiled.status.success() {
        return (
            format!("compile_error\n{compile_ms} 0\n"),
            String::from_utf8_lossy(&compiled.stderr).into_owned(),
        );
    }
    let main = match load(so) {
        Ok(main) => main,
        Err((status, err)) => return (format!("{status}\n{compile_ms} 0\n"), err),
    };
    let t1 = Instant::now();
    let captured = so.with_extension("out");
    // The default panic hook already prints the message into the capture.
    let status = with_output_to(&captured, || {
        match catch_unwind(AssertUnwindSafe(|| main(ctx))) {
            Ok(()) => "ok",
            Err(_) => "panic",
        }
    });
    let run_ms = t1.elapsed().as_millis();
    let out = fs::read_to_string(&captured).unwrap_or_default();
    (format!("{status}\n{compile_ms} {run_ms}\n"), out)
}

type Main<T> = fn(&mut T);

/// `dlopen` the snippet and check `pry_type` matches `T`; `Err((status, message))`.
fn load<T>(so: &Path) -> Result<Main<T>, (&'static str, String)> {
    let c_path = CString::new(so.as_os_str().as_encoded_bytes()).expect("path");
    // Kept loaded so pointers and allocations from snippet code remain valid.
    let handle = unsafe { dlopen(c_path.as_ptr(), RTLD_NOW) };
    if handle.is_null() {
        return Err(("load_error", format!("dlopen: {}\n", dl_error())));
    }
    let sym = |name: &str| {
        let c = CString::new(name).expect("name");
        unsafe { dlsym(handle, c.as_ptr()) }
    };
    let (ty, main) = (sym("pry_type"), sym("pry_main"));
    if ty.is_null() || main.is_null() {
        return Err((
            "load_error",
            "missing pry_type/pry_main: wrap the snippet in pry::snippet!\n".into(),
        ));
    }
    let ty: fn() -> &'static str = unsafe { std::mem::transmute(ty) };
    let (want, got) = (type_name::<T>(), ty());
    if want != got {
        return Err((
            "type_mismatch",
            format!("host ctx is `{want}`, snippet wants `{got}`\n"),
        ));
    }
    Ok(unsafe { std::mem::transmute::<*mut c_void, Main<T>>(main) })
}

fn dl_error() -> String {
    let p = unsafe { dlerror() };
    if p.is_null() {
        return "unknown".into();
    }
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}

/// Run `f` with fds 1 and 2 pointing at `path`, then restore them.
fn with_output_to<R>(path: &Path, f: impl FnOnce() -> R) -> R {
    use std::os::fd::AsRawFd;
    let file = fs::File::create(path).expect("pry: capture file");
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
    let saved = [unsafe { dup(1) }, unsafe { dup(2) }];
    for fd in [1, 2] {
        unsafe { dup2(file.as_raw_fd(), fd) };
    }
    let r = f();
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
    for (fd, s) in [1, 2].into_iter().zip(saved) {
        unsafe {
            dup2(s, fd);
            close(s);
        }
    }
    r
}

/// `--extern name=path` for every crate in the host's `target/<profile>/deps`,
/// newest file per name (ties between `.so` and `.rlib` go to the dylib).
/// Lets snippets `use` any dependency the host has, with matching metadata.
fn externs_from_deps() -> Vec<String> {
    let exe = std::env::current_exe().expect("current_exe");
    let deps = exe.parent().expect("exe dir").join("deps");
    let mut newest: std::collections::HashMap<String, (std::time::SystemTime, bool, PathBuf)> =
        Default::default();
    for e in fs::read_dir(&deps)
        .expect("pry: no deps dir next to the executable")
        .flatten()
    {
        let p = e.path();
        let Some(ext) = p.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if ext != "rlib" && ext != "so" {
            continue;
        }
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // Workspace dylibs are unhashed (`libapp.so`), the rest `libfoo-HASH.rlib`.
        let Some(name) = stem
            .strip_prefix("lib")
            .map(|s| s.rsplit_once('-').map_or(s, |(n, _)| n))
        else {
            continue;
        };
        let mtime = e
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::UNIX_EPOCH);
        let key = (mtime, ext == "so");
        match newest.get(name) {
            Some((t, so, _)) if (*t, *so) >= key => {}
            _ => {
                newest.insert(name.to_owned(), (mtime, ext == "so", p));
            }
        }
    }
    let mut flags = vec![format!("-Ldependency={}", deps.display())];
    for (name, (_, _, p)) in newest {
        flags.push(format!("--extern={name}={}", p.display()));
    }
    flags
}
