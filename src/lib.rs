use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    sync::mpsc::{channel, Receiver, Sender},
    time::{Duration, SystemTime},
};

pub trait Message: Sized + Debug + Clone + 'static {}

pub trait Tag: Sized + Eq + Hash + Debug + Clone + 'static {}

impl Tag for String {}

pub type Millis = u64;

pub trait Actor: Sized + Debug {
    type T: Tag;
    type M: Message;
    fn act(&mut self, tag: &Self::T, ctx: &mut Context<Self::T, Self, Self::M>, msg: Self::M);
}

pub struct Context<T: Tag, A: Actor, M: Message> {
    tx: Sender<Action<T, A, M>>,
    now: Millis,
}

#[derive(Debug)]
pub enum Action<T: Tag, A: Actor, M: Message> {
    Bind(T, A),
    Send(T, M),
    Post(T, M, Millis),
    Stop(T),
}

impl<T: Tag, A: Actor, M: Message> Context<T, A, M> {
    fn new(tx: Sender<Action<T, A, M>>) -> Self {
        Self { tx, now: 0 }
    }
}

impl<T: Tag, A: Actor, M: Message> Context<T, A, M> {
    pub fn stop(&mut self, tag: &T) {
        self.tx.send(Action::Stop(tag.clone())).unwrap();
    }

    pub fn send(&mut self, tag: &T, msg: M) {
        self.tx.send(Action::Send(tag.clone(), msg)).unwrap();
    }

    pub fn bind(&mut self, tag: T, actor: A) {
        self.tx.send(Action::Bind(tag, actor)).unwrap();
    }

    pub fn post(&mut self, tag: T, msg: M, millis: Millis) {
        self.tx.send(Action::Post(tag, msg, millis)).unwrap();
    }

    pub fn now(&self) -> Millis {
        self.now
    }
}

struct Post<T: Tag, M: Message>(Millis, T, M);

impl<T: Tag, M: Message> PartialEq for Post<T, M> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Tag, M: Message> Eq for Post<T, M> {}

impl<T: Tag, M: Message> PartialOrd for Post<T, M> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Tag, M: Message> Ord for Post<T, M> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

pub struct System<T: Tag, A: Actor, M: Message> {
    actors: HashMap<T, A>,
    queues: HashMap<T, VecDeque<M>>,
    posted: BinaryHeap<Post<T, M>>,
    millis: Millis,
    tx: Sender<Action<T, A, M>>,
    rx: Receiver<Action<T, A, M>>,
}

impl<T: Tag, A: Actor, M: Message> Default for System<T, A, M> {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            actors: Default::default(),
            queues: Default::default(),
            posted: Default::default(),
            millis: 0,
            tx,
            rx,
        }
    }
}

impl<T: Tag, A: Actor<T = T, M = M>, M: Message> System<T, A, M> {
    pub fn context(&self) -> Context<T, A, M> {
        Context::new(self.tx.clone())
    }

    pub fn run(&mut self) {
        action_loop(self, get_current_millis);
    }
}

fn get_current_millis() -> Millis {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as Millis
}

fn handle_actions<T: Tag, A: Actor<T = T, M = M>, M: Message>(sys: &mut System<T, A, M>) {
    while let Ok(action) = sys.rx.recv_timeout(Duration::from_millis(0)) {
        match action {
            Action::Bind(tag, actor) => {
                sys.actors.insert(tag, actor);
            }
            Action::Send(tag, msg) => {
                sys.queues.entry(tag).or_default().push_back(msg);
            }
            Action::Post(tag, msg, mut millis) => {
                millis += sys.millis;
                sys.posted.push(Post(millis, tag, msg));
            }
            Action::Stop(tag) => {
                sys.actors.remove(&tag);
            }
        }
    }
}

fn handle_posts<T: Tag, A: Actor<T = T, M = M>, M: Message>(
    sys: &mut System<T, A, M>,
    ctx: &mut Context<T, A, M>,
) {
    if sys.posted.is_empty() {
        return;
    }

    while sys
        .posted
        .peek()
        .map(|Post(deadline, _, _)| deadline)
        .cloned()
        .unwrap_or(Millis::MAX)
        <= sys.millis
    {
        if let Some(Post(_, tag, msg)) = sys.posted.pop() {
            if let Some(actor) = sys.actors.get_mut(&tag) {
                actor.act(&tag, ctx, msg);
            }
        }
    }
}

fn handle_actors<T: Tag, A: Actor<T = T, M = M>, M: Message>(
    sys: &mut System<T, A, M>,
    ctx: &mut Context<T, A, M>,
) {
    sys.queues
        .iter_mut()
        .filter(|(_, queue)| !queue.is_empty())
        .map(|(tag, queue)| (tag, queue.pop_front()))
        .for_each(|(tag, msg)| {
            if let Some(msg) = msg {
                if let Some(actor) = sys.actors.get_mut(tag) {
                    actor.act(tag, ctx, msg);
                }
            }
        });
}

fn action_loop<T: Tag, A: Actor<T = T, M = M>, M: Message, F: Fn() -> Millis>(
    sys: &mut System<T, A, M>,
    clock: F,
) {
    let mut ctx = sys.context();
    loop {
        sys.millis = clock();
        ctx.now = sys.millis;

        handle_actions(sys);
        handle_posts(sys, &mut ctx);
        handle_actors(sys, &mut ctx);

        if sys.actors.is_empty() {
            break;
        }
    }
}
