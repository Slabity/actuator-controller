#![feature(impl_trait_in_assoc_type)]
#![no_std]
#![no_main]

use embassy_executor;
use embassy_rp::{self, bind_interrupts, gpio, multicore, peripherals, usb, Peri};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Sender};
use embassy_time::{Duration, Timer};
use embassy_usb::class::cdc_acm::{CdcAcmClass, Sender as CdcSender, State as CdcState};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod adc;

static mut CORE1_STACK: multicore::Stack<4096> = multicore::Stack::new();
static CORE1_EXEC: StaticCell<embassy_executor::Executor> = StaticCell::new();

static USB_CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_MSOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static USB_CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
static USB_CDC_STATE: StaticCell<CdcState> = StaticCell::new();

static ADC_CHANNEL: Channel<CriticalSectionRawMutex, i32, 4> = Channel::new();

pub type ReadingsSender = Sender<'static, CriticalSectionRawMutex, i32, 4>;
type UsbDriver = usb::Driver<'static, peripherals::USB>;
type UsbTx = CdcSender<'static, UsbDriver>;

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<peripherals::USB>;
});

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());

    // Give program a moment before starting fully
    Timer::after_millis(1000).await;
    defmt::info!("Starting program");

    // Set up USB CDC serial
    let driver = usb::Driver::new(p.USB, Irqs);
    let mut usb_config = embassy_usb::Config::new(0xc0de, 0xcafe);
    usb_config.manufacturer = Some("Slabity");
    usb_config.product = Some("Actuator Controller");
    usb_config.serial_number = Some("01");
    usb_config.device_class = 0xEF;
    usb_config.device_sub_class = 0x02;
    usb_config.device_protocol = 0x01;
    usb_config.composite_with_iads = true;

    let mut builder = embassy_usb::Builder::new(
        driver,
        usb_config,
        USB_CONFIG_DESC.init([0; 256]),
        USB_BOS_DESC.init([0; 256]),
        USB_MSOS_DESC.init([0; 256]),
        USB_CONTROL_BUF.init([0; 64]),
    );
    let cdc = CdcAcmClass::new(&mut builder, USB_CDC_STATE.init(CdcState::new()), 64);
    let usb = builder.build();
    let (cdc_tx, _) = cdc.split();

    spawner.spawn(usb_run_task(usb).unwrap());
    spawner.spawn(usb_serial_task(cdc_tx).unwrap());

    defmt::info!("Spawning ADC process");
    let adc_r = adc::AdcResources {
        sda: p.PIN_14,
        scl: p.PIN_15,
        i2c: p.I2C1,
        interrupt: p.PIN_13,
        readings_tx: ADC_CHANNEL.sender(),
    };
    multicore::spawn_core1(
        p.CORE1,
        unsafe { &mut *core::ptr::addr_of_mut!(CORE1_STACK) },
        move || {
            let executor = CORE1_EXEC.init(embassy_executor::Executor::new());
            executor.run(|spawner| spawner.spawn(adc::adc_task(adc_r).unwrap()));
        },
    );

    defmt::info!("Spawning LED process");
    spawner.spawn(led_looper(p.PIN_25).unwrap());

    defmt::info!("Entering main loop");
    loop {
        Timer::after(Duration::from_millis(1000)).await;
        defmt::info!("Looping");
    }
}

#[embassy_executor::task]
async fn usb_run_task(mut device: embassy_usb::UsbDevice<'static, UsbDriver>) {
    device.run().await;
}

#[embassy_executor::task]
async fn usb_serial_task(mut tx: UsbTx) {
    use core::fmt::Write;
    let rx = ADC_CHANNEL.receiver();
    loop {
        tx.wait_connection().await;
        defmt::info!("USB connected");
        loop {
            let position = rx.receive().await;
            let mut buf: heapless::String<32> = heapless::String::new();
            write!(buf, "{}\n", position).ok();
            if tx.write_packet(buf.as_bytes()).await.is_err() {
                defmt::info!("USB disconnected");
                break;
            }
        }
    }
}

#[embassy_executor::task]
async fn led_looper(pin: Peri<'static, peripherals::PIN_25>) {
    let mut led = gpio::Output::new(pin, gpio::Level::Low);

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(250)).await;

        led.set_low();
        Timer::after(Duration::from_millis(750)).await;
    }
}
