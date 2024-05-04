#![feature(impl_trait_in_assoc_type)]
#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use embassy_executor;
use embassy_futures;

use embassy_rp;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals;
use embassy_rp::multicore;
use embassy_rp::usb;
use embassy_rp::gpio;

use embassy_time::{Duration, Timer};

use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

mod adc;

static mut CORE1_STACK: multicore::Stack<4096> = multicore::Stack::new();
static mut CORE1_EXEC: StaticCell<embassy_executor::Executor> = StaticCell::new();

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<peripherals::USB>;
});

assign_resources::assign_resources! {
    adc: AdcResources {
        sda: PIN_16,
        scl: PIN_17,
        i2c: I2C0,
        interrupt: PIN_18
    }
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    embassy_rp::pac::SIO.spinlock(31).write_value(1);
    let p = embassy_rp::init(Default::default());
    let mut led = gpio::Output::new(p.PIN_25, gpio::Level::Low);
    let r = split_resources!(p);

    // Give program a moment before starting fully
    Timer::after_millis(1000).await;
    defmt::info!("Starting program");

    defmt::info!("Spawning ADC process");
    multicore::spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor = unsafe {
                CORE1_EXEC.init(embassy_executor::Executor::new())
            };
            executor.run(|spawner| spawner.spawn(adc::adc_task(r.adc)).unwrap());
        }
    );

    defmt::info!("Entering main loop");
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(250)).await;

        led.set_low();
        Timer::after(Duration::from_millis(750)).await;

        embassy_futures::yield_now().await
    }
}


