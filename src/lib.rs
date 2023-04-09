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

pub trait Actor: Sized + Debug {
    type T: Tag;
    type M: Message;
    fn act(&mut self, tag: &Self::T, ctx: &mut Context<Self::T, Self, Self::M>, msg: Self::M);
}

pub struct Context<T: Tag, A: Actor, M: Message> {
    tx: Sender<Action<T, A, M>>,
    now: u32,
}

#[derive(Debug)]
pub enum Action<T: Tag, A: Actor, M: Message> {
    Bind(T, A),
    Send(T, M),
    Post(T, M, u32),
    Stop(T),
}

impl<T: Tag, A: Actor, M: Message> Context<T, A, M> {
    pub fn with_sender(tx: Sender<Action<T, A, M>>) -> Self {
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

    pub fn post(&mut self, tag: &T, msg: M, millis: u32) {
        self.tx
            .send(Action::Post(tag.clone(), msg, millis))
            .unwrap();
    }

    pub fn now(&self) -> u32 {
        self.now
    }
}

struct Post<T: Tag, M: Message>(u32, T, M);

impl<T: Tag, M: Message> PartialEq for Post<T, M> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: Tag, M: Message> Eq for Post<T, M> {}

impl<T: Tag, M: Message> PartialOrd for Post<T, M> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T: Tag, M: Message> Ord for Post<T, M> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.0.partial_cmp(&self.0).unwrap()
    }
}

pub struct System<T: Tag, A: Actor, M: Message> {
    actors: HashMap<T, A>,
    queues: HashMap<T, VecDeque<M>>,
    posted: BinaryHeap<Post<T, M>>,
    millis: u32,
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
            millis: Default::default(),
            tx,
            rx,
        }
    }
}

impl<T: Tag, A: Actor<T = T, M = M>, M: Message> System<T, A, M> {
    pub fn context(&self) -> Context<T, A, M> {
        Context::with_sender(self.tx.clone())
    }

    pub fn run(&mut self) {
        let clock = || {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u32
        };
        action_loop(self, clock);
    }
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
        .unwrap_or(u32::MAX)
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

fn action_loop<T: Tag, A: Actor<T = T, M = M>, M: Message, F: Fn() -> u32>(
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
