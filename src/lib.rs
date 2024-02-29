use std::{
    num::NonZeroUsize,
    sync::{mpsc, Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};

#[derive(Clone)]
pub struct ThreadPool {
    workers: Arc<[Worker]>,
    message_queue: mpsc::Sender<Message>,
    queue_has_message: Arc<Condvar>,
}

struct Worker {
    id: usize,
    handle: Option<JoinHandle<()>>,
}

enum Message {
    Task(Task<'static>),
    Terminate,
}

type Task<'a> = Box<dyn FnOnce() + Send + 'a>;

impl<'a> Default for ThreadPool {
    fn default() -> Self {
        Self::new(thread::available_parallelism().unwrap())
    }
}

impl ThreadPool {
    pub fn new(size: NonZeroUsize) -> Self {
        let (message_sender, message_receiver) = mpsc::channel();
        let message_receiver = Arc::new(Mutex::new(message_receiver));
        let queue_has_message = Arc::new(Condvar::new());

        let workers = (0..size.get())
            .map(|id| {
                Worker::new(
                    id,
                    Arc::clone(&message_receiver),
                    Arc::clone(&queue_has_message),
                )
            })
            .collect();

        Self {
            workers,
            message_queue: message_sender,
            queue_has_message,
        }
    }

    pub fn run<F: FnOnce() + Send + 'static>(&self, task: F) {
        let task = Box::new(task);
        self.message_queue.send(Message::Task(task)).unwrap();
        self.queue_has_message.notify_one();
    }

    pub fn size(&self) -> usize {
        self.workers.len()
    }
}

impl Worker {
    fn new(
        id: usize,
        message_queue: Arc<Mutex<mpsc::Receiver<Message>>>,
        cv: Arc<Condvar>,
    ) -> Self {
        Self {
            id,
            handle: Some(
                thread::Builder::new()
                    .name(format!("TreadPool::Worker {id}"))
                    .spawn(move || {
                        'l: loop {
                            let mut queue = message_queue.lock().unwrap();
                            let message = loop {
                                match queue.try_recv() {
                                    Ok(message) => break message,
                                    Err(err) => match err {
                                        mpsc::TryRecvError::Empty => {
                                            let new_queue = match cv.wait(queue) {
                                                Ok(new_queue) => new_queue,
                                                Err(err) => err.into_inner(),
                                            };
                                            queue = new_queue;
                                        }
                                        mpsc::TryRecvError::Disconnected => break 'l,
                                    },
                                }
                            };
                            drop(queue);
                            match message {
                                Message::Task(task) => {
                                    eprintln!("Worker {id} received a task; executing...");
                                    task();
                                }
                                Message::Terminate => {
                                    eprintln!("Worker {id} received a termination message.");
                                    break;
                                }
                            }
                        }
                        eprintln!("Terminating worker {id}...");
                    })
                    .unwrap(),
            ),
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        let Some(workers) = Arc::get_mut(&mut self.workers) else {
            return;
        };

        for _ in 0..workers.len() {
            self.message_queue.send(Message::Terminate).unwrap();
            self.queue_has_message.notify_one();
        }
        for Worker { id, handle } in workers.iter_mut() {
            if let Some(handle) = handle.take() {
                handle.join().unwrap();
            }
            eprintln!("Stopped worker {id}.")
        }
    }
}
