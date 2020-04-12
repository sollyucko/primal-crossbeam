use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use primal::{estimate_prime_pi, Primes, Sieve};
use std::thread;

pub fn thread_spawn<T>(result: (impl FnOnce() + Send + 'static, T)) -> (thread::JoinHandle<()>, T) {
    let (f, x) = result;
    (thread::spawn(f), x)
}

fn from_iterator<'a, T: Send + 'a>(
    it: impl Iterator<Item = T> + Send + 'a,
    s: Sender<T>,
    r: Receiver<T>,
) -> (impl FnOnce() + Send + 'a, Receiver<T>) {
    (
        move || {
            for x in it {
                if s.send(x).is_err() {
                    return;
                }
            }
        },
        r,
    )
}

fn from_iterator_unbounded<'a, T: Send + 'a>(
    it: impl Iterator<Item = T> + Send + 'a,
) -> (impl FnOnce() + Send + 'a, Receiver<T>) {
    let (s, r) = unbounded::<T>();
    from_iterator(it, s, r)
}

/// Assumes that `it` has at most `bound` values.
fn from_iterator_bounded<'a, T: Send + 'a>(
    it: impl Iterator<Item = T> + Send + 'a,
    bound: usize,
) -> (impl FnOnce() + 'a, Receiver<T>) {
    let (s, r) = bounded::<T>(bound);
    from_iterator(it, s, r)
}

/// ```
/// # use primal_crossbeam::*;
/// # use std::thread;
/// let (thread, r) = thread_spawn(primes_unbounded());
/// assert_eq!(r.recv(), Ok(2));
/// assert_eq!(r.recv(), Ok(3));
/// assert_eq!(r.recv(), Ok(5));
/// // thread.join(); // Would block indefinitely
/// drop(r);
/// thread.join();
pub fn primes_unbounded() -> (impl FnOnce() + Send, Receiver<usize>) {
    from_iterator_unbounded(Primes::all())
}

struct WithObj<T, U> {
    pub value: T,
    #[allow(dead_code)]
    obj_box: Box<U>,
}

impl<T, U> WithObj<T, U> {
    fn new<'a, F>(obj: U, make_value: F) -> Self
    where
        U: 'a,
        F: FnOnce(&'a U) -> T + 'a,
    {
        let obj_box = Box::new(obj);
        Self {
            value: make_value(unsafe { &*(&*obj_box as *const U) }),
            obj_box,
        }
    }
}

impl<T, U, V> Iterator for WithObj<T, U>
where
    T: Iterator<Item = V>,
{
    type Item = V;

    fn next(&mut self) -> Option<V> {
        self.value.next()
    }
}

/// ```
/// # use primal_crossbeam::*;
/// # use std::thread;
/// let (thread, r) = thread_spawn(primes_bounded(5));
/// assert_eq!(r.recv(), Ok(2));
/// assert_eq!(r.recv(), Ok(3));
/// thread.join(); // Since the number of primes is bounded, this will eventually terminate.
/// assert_eq!(r.recv(), Ok(5));
pub fn primes_bounded(limit: usize) -> (impl FnOnce() + Send, Receiver<usize>) {
    let (_, high) = estimate_prime_pi(limit as u64);
    #[allow(clippy::cast_possible_truncation)]
    let len = high as usize;
    let sieve = Sieve::new(limit);
    from_iterator_bounded(
        WithObj::new(sieve, move |s| {
            Sieve::primes_from(s, 0).take_while(move |x| x <= &limit)
        }),
        len,
    )
}
