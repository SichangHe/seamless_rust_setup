use app::State;

pry::snippet!(|_st: &mut State| {
    panic!("intentional snippet panic");
});
