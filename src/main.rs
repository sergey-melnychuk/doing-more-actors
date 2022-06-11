#![feature(unboxed_closures)]

use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

use std::collections::HashMap;

//

// T = tag, A = actor, M = message, F = [actor] factory

trait System<T, M, F, A> 
    where 
        Self: Sized,
        T: Hash + Sized + Clone + Debug + 'static,
        M: Sized + Clone + Debug + 'static, // + Serializable + Deserializable
        A: Actor<T, M, F, Self>,
        F: Fn() -> A,
{
    fn send(&self, tag: &T, message: M);
    fn spawn(&self, tag: &T, f: F);
    fn stop(&self, tag: &T);
    fn delay(&self, tag: &T, message: M, millis: u64);
    fn now(&self) -> u64;
}

trait Actor<T, M, F, S>
    where 
        Self: Sized,
        T: Hash + Sized + Clone + Debug + 'static,
        M: Sized + Clone + Debug + 'static, // + Serializable + Deserializable
        F: Fn() -> Self,
        S: System<T, M, F, Self>,
{
    fn act(&mut self, tag: &T, message: M, system: &S);
}

// ---

#[derive(Default)]
struct Config;

#[derive(Default)]
struct Threads;

#[derive(Default)]
struct ActorSystem<T, M, F, A> 
    where 
        Self: Sized,
        T: Hash + Sized + Clone + Debug + 'static,
        M: Sized + Clone + Debug + 'static, // + Serializable + Deserializable
        F: Fn() -> A,
        A: Actor<T, M, F, Self>,
{
    config: Config,
    actors: HashMap<T, A>,
    threads: Threads,
    x: PhantomData<(M, F)>,
}

impl<T, A, M, F> System<T, M, F, A> for ActorSystem<T, M, F, A>
    where 
        Self: Sized,
        T: Hash + Sized + Clone + Debug + 'static,
        M: Sized + Clone + Debug + 'static, // + Serializable + Deserializable
        F: Fn() -> A,
        A: Actor<T, M, F, Self>,
{
    fn send(&self, tag: &T, message: M) {}
    fn spawn(&self, tag: &T, f: F) {}
    fn stop(&self, tag: &T) {}
    fn delay(&self, tag: &T, message: M, millis: u64) {}    
    fn now(&self) -> u64 { 0 }
}

// ---

enum SomeActor {
    Dummy,
}

impl<T, M, F, S> Actor<T, M, F, S> for SomeActor
    where 
        Self: Sized,
        T: Hash + Sized + Clone + Debug + 'static,
        M: Sized + Clone + Debug + 'static, // + Serializable + Deserializable
        F: Fn() -> Self,
        S: System<T, M, F, Self>,
{
    fn act(&mut self, tag: &T, message: M, system: &S) {
        println!("never ending story: {:?}", message);
        system.send(tag, message);    
    }
}

// ---

struct X;

fn main() {
    let mut map = HashMap::with_capacity(1);
    map.insert("x", SomeActor::Dummy);

    let system = ActorSystem::default();
    system.send(&"".to_string(), "".to_string());
    system.spawn(&"".to_string(), || SomeActor::Dummy);

    println!("done");
}

