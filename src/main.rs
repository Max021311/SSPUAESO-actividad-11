use std::fmt;
use std::time::Duration;
use std::{io, sync::Arc};

use rand::distributions::uniform::{SampleRange, SampleUniform};
use rand::Rng;

use tokio::runtime::Builder;
use tokio::sync::{Mutex, Semaphore};
use tokio::task;
#[cfg(not(feature = "no-sleep"))]
use tokio::time::sleep;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute, style,
    terminal::{disable_raw_mode, enable_raw_mode},
    tty::IsTty,
};

const BUFFER_SIZE: usize = 25;

struct Buffer {
    data: [u8; BUFFER_SIZE],
    amount: usize,
    push_index: usize,
    pop_index: usize,
}

impl Buffer {
    fn new() -> Self {
        Buffer {
            data: [0; 25],
            amount: 0,
            push_index: 0,
            pop_index: 0,
        }
    }

    fn push(&mut self, amount: usize) -> usize {
        for i in 1..=amount {
            if self.amount >= self.data.len() {
                return i - 1;
            }
            if self.push_index >= self.data.len() {
                self.push_index = 0
            }
            self.data[self.push_index] = 1;
            self.push_index += 1;
            self.amount += 1;
        }
        amount
    }

    fn pop(&mut self, amount: usize) -> usize {
        for i in 1..=amount {
            if self.amount <= 0 {
                return i - 1;
            }
            if self.pop_index >= self.data.len() {
                self.pop_index = 0
            }
            self.data[self.pop_index] = 0;
            self.pop_index += 1;
            self.amount -= 1;
        }
        amount
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.amount == 0
    }

    #[inline(always)]
    fn is_full(&self) -> bool {
        self.amount == self.data.len()
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Cantidad: {}\r\n", self.amount)?;
        for number in self.data {
            write!(f, "{:3}", number)?;
        }
        write!(f, "\r\n")?;
        for i in 1..=25 {
            write!(f, "{:3}", i)?;
        }
        write!(f, "\r\n")
    }
}

fn gen_range<T, R>(range: R) -> T
where
    T: SampleUniform,
    R: SampleRange<T>,
{
    let mut rng = rand::thread_rng();
    rng.gen_range::<T, R>(range)
}

async fn producer_task(
    buffer: Arc<Mutex<Buffer>>,
    semaphore: Arc<Semaphore>,
    exit_flag: Arc<Mutex<bool>>,
) -> Result<(), io::Error> {
    execute!(
        io::stdout().lock(),
        style::ResetColor,
        style::SetForegroundColor(style::Color::Green),
        style::Print("Iniciando productor\r\n\r\n")
    )?;
    let mut counter = 0;
    loop {
        let flag = { *exit_flag.lock().await };
        if flag {
            break;
        }
        if counter >= BUFFER_SIZE * 2 {
            break;
        }

        // If true yield now and return the control to the scheduler
        let flag = gen_range(0..2) == 1;
        if flag {
            let mut stdout = io::stdout().lock();
            execute!(
                stdout,
                style::ResetColor,
                style::SetForegroundColor(style::Color::Green),
                style::Print("Productor intentando acceder al buffer\r\n")
            )?;
            //if let (Ok(_), Ok(mut lock)) = (semaphore.try_acquire(), buffer.try_lock())  {
            if let (Ok(_), Ok(mut lock)) = (semaphore.try_acquire(), buffer.try_lock()) {
                if lock.is_full() {
                    execute!(
                        stdout,
                        style::ResetColor,
                        style::SetForegroundColor(style::Color::Green),
                        style::Print("Buffer lleno\r\n")
                    )?;
                } else {
                    execute!(
                        stdout,
                        style::ResetColor,
                        style::SetForegroundColor(style::Color::Green),
                        style::Print("Productor trabajando.\r\n")
                    )?;
                    let amount = lock.push(2);
                    counter += 1;
                    execute!(
                        stdout,
                        style::ResetColor,
                        style::SetForegroundColor(style::Color::Green),
                        style::Print(std::format!("Produjo: {amount}\r\n"))
                    )?;
                }
                execute!(
                    stdout,
                    style::Print(format!("{lock}\r\n")),
                    style::Print("Productor durmiendo\r\n")
                )?;
            } else {
                execute!(
                    stdout,
                    style::ResetColor,
                    style::SetForegroundColor(style::Color::Green),
                    style::Print("Productor no pudo acceder al buffer\r\n")
                )?;
            }
            execute!(stdout, style::Print("\r\n"))?;
        }
        #[cfg(not(feature = "no-sleep"))]
        sleep(Duration::from_millis(500)).await;
        task::yield_now().await;
    }
    execute!(
        io::stdout().lock(),
        style::ResetColor,
        style::SetForegroundColor(style::Color::Green),
        style::Print("Terminando productor\r\n\r\n"),
        style::ResetColor
    )?;
    return Ok(());
}

async fn consumer_task(
    buffer: Arc<Mutex<Buffer>>,
    semaphore: Arc<Semaphore>,
    exit_flag: Arc<Mutex<bool>>,
) -> Result<(), io::Error> {
    execute!(
        io::stdout().lock(),
        style::ResetColor,
        style::SetForegroundColor(style::Color::Red),
        style::Print("Iniciando consumidor\r\n\r\n")
    )?;
    let mut counter = 0;
    loop {
        let flag = { *exit_flag.lock().await };
        if flag {
            break;
        }

        if counter >= BUFFER_SIZE * 2 {
            break;
        }

        let flag = gen_range(0..2) == 1;
        if flag {
            let mut stdout = io::stdout().lock();
            execute!(
                stdout,
                style::ResetColor,
                style::SetForegroundColor(style::Color::Red),
                style::Print("Consumidor intentando acceder al buffer\r\n")
            )?;
            if let (Ok(_), Ok(mut lock)) = (semaphore.try_acquire(), buffer.try_lock()) {
                if lock.is_empty() {
                    execute!(
                        stdout,
                        style::ResetColor,
                        style::SetForegroundColor(style::Color::Red),
                        style::Print("Buffer vacío\r\n")
                    )?;
                } else {
                    execute!(
                        stdout,
                        style::ResetColor,
                        style::SetForegroundColor(style::Color::Red),
                        style::Print("Consumidor trabajando.\r\n")
                    )?;
                    // TODO: Consume two elements
                    let amount = lock.pop(2);
                    counter += 1;
                    execute!(
                        stdout,
                        style::ResetColor,
                        style::SetForegroundColor(style::Color::Red),
                        style::Print(std::format!("Consumio: {amount}\r\n"))
                    )?;
                }
                execute!(
                    stdout,
                    style::Print(format!("{lock}\r\n")),
                    style::Print("Consumidor durmiendo\r\n")
                )?;
            } else {
                execute!(
                    stdout,
                    style::ResetColor,
                    style::SetForegroundColor(style::Color::Red),
                    style::Print("Consumidor no pudo acceder al buffer\r\n")
                )?;
            }
            execute!(stdout, style::Print("\r\n"))?;
        }
        #[cfg(not(feature = "no-sleep"))]
        sleep(Duration::from_millis(500)).await;
        task::yield_now().await;
    }
    execute!(
        io::stdout().lock(),
        style::ResetColor,
        style::SetForegroundColor(style::Color::Red),
        style::Print("Terminando consumidor\r\n\r\n"),
        style::ResetColor
    )?;
    Ok(())
}

async fn input_handler(exit_flag: Arc<Mutex<bool>>) -> Result<(), io::Error> {
    loop {
        if *exit_flag.lock().await {
            break;
        }
        if let Ok(true) = event::poll(Duration::from_millis(100)) {
            if let Ok(Event::Key(key_event)) = event::read() {
                match key_event {
                    event::KeyEvent {
                        code: KeyCode::Esc, .. // At press Escape
                    }
                    | event::KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL, // At press ctrl-c
                        ..
                    } => {
                        let mut flag = exit_flag.lock().await;
                        *flag = true;
                        break;
                    }
                    _ => { /* Discard other key events */ }
                }
            }
        }
        //#[cfg(feature = "one-thread")]
        task::yield_now().await;
    }
    Ok(())
}

fn main() {
    // NOTE: if the compiler flag `one-thread` is enabled use conncurrency in one thread instead of
    // multiple threads
    let mut runtime = if cfg!(feature = "one-thread") {
        Builder::new_current_thread()
    } else {
        Builder::new_multi_thread()
    };

    let runtime = runtime.enable_time()
        .worker_threads(3)
        .build().unwrap();

    runtime.block_on(async {
        if !io::stdout().is_tty() {
            eprintln!("Terminando programa. Es necesario ejecutarlo en una terminal");
            return ();
        }
        println!("Presiona \"escape\" o \"ctrl-c\" para salir del programa\r\n");

        enable_raw_mode().unwrap();

        let buffer = Arc::new(Mutex::new(Buffer::new()));
        // NOTE: The semaphore is unnecessary because the mutex use a semaphore with one permit
        // behind to ensure the mutual exclusion but i leave it in to show how to use it
        let semaphore = Arc::new(Semaphore::new(1));
        let exit_flag = Arc::new(Mutex::new(false));

        let producer_handle = task::spawn(producer_task(
            buffer.clone(),
            semaphore.clone(),
            exit_flag.clone(),
        ));
        let consumer_handle = task::spawn(consumer_task(
            buffer.clone(),
            semaphore.clone(),
            exit_flag.clone(),
        ));
        let input_handle = task::spawn(input_handler(exit_flag.clone()));

        let _ = tokio::join!(producer_handle, consumer_handle);
        input_handle.abort();
        let _ = input_handle.await;
        disable_raw_mode().unwrap();
    });
}
