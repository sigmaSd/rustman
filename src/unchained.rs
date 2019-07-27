use std::thread;

pub trait Unchained
where
    Self: Iterator + Sized,
{
    fn unchained_for_each<F>(self, f: F) -> UnchainedForEach<Self, F>
    where
        F: FnMut(Self::Item) + Send + Sync + 'static,
    {
        UnchainedForEach::new(self, f)
    }
}

impl<I: Iterator> Unchained for I {}

pub struct UnchainedForEach<I: Iterator, F: FnMut(I::Item) + Send + Sync + 'static> {
    iter: I,
    f: F,
}

impl<I: Iterator, F: FnMut(I::Item) + Send + Sync + 'static> UnchainedForEach<I, F> {
    fn new(iter: I, f: F) -> Self {
        Self { iter, f }
    }
}

impl<I: Iterator, F> Iterator for UnchainedForEach<I, F>
where
    F: FnMut(I::Item) + Sized + Send + Sync + Clone + 'static,
    I::Item: Send + 'static,
{
    type Item = thread::JoinHandle<()>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.iter.next() {
            Some(next) => next,
            None => return None,
        };
        let mut f = self.f.clone();
        Some(thread::spawn(move || {
            f(next);
        }))
    }
}
pub trait Finisher {
    fn join(self);
}
impl<T> Finisher for T
where
    T: Iterator<Item = thread::JoinHandle<()>>,
{
    fn join(self) {
        self.collect::<Vec<thread::JoinHandle<()>>>()
            .into_iter()
            .for_each(|t| {
                let _ = t.join();
            })
    }
}
