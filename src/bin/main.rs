use doing_more_actors::{Actor, Context, Message, System};

#[derive(Debug, Clone)]
enum Protocol {
    Empty,
}

impl Message for Protocol {}

#[derive(Debug)]
enum State {
    Empty,
    Counter(usize, u32),
    Done,
}

impl Actor for State {
    type T = String;
    type M = Protocol;

    fn act(&mut self, tag: &Self::T, ctx: &mut Context<Self::T, Self, Self::M>, msg: Self::M) {
        println!("[tag={:?}] state={:?} message={:?}", tag, self, msg);
        match self {
            State::Empty => {
                ctx.send(tag, msg);
                *self = State::Counter(0, ctx.now());
            }
            State::Counter(10, time) => {
                println!("time: {}", ctx.now() - *time);
                *time = ctx.now();
                ctx.send(tag, msg);
                *self = State::Done;
            }
            State::Counter(n, time) => {
                println!("time: {}", ctx.now() - *time);
                *time = ctx.now();
                *n += 1;
                ctx.post(tag, msg, 100);
            }
            State::Done => ctx.stop(tag),
        }
    }
}

fn main() {
    let mut sys = System::default();

    let mut ctx = sys.context();
    ctx.bind("tag".to_owned(), State::Empty);
    ctx.send(&"tag".to_string(), Protocol::Empty);

    sys.run();
}
