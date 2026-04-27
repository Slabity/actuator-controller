use embassy_rp::{gpio, i2c, peripherals, Peri};

use ads1x1x;

// Number of values we want to read on a channel before switching.
const AVG_WINDOW: usize = 8;

// Number of extra values to read that will be cutoff.
const AVG_CUTOFF: usize = 2;

// Full size of the array to store readings in.
const AVG_LENGTH: usize = AVG_WINDOW + (AVG_CUTOFF * 2);

pub struct AdcResources {
    pub sda: Peri<'static, peripherals::PIN_14>,
    pub scl: Peri<'static, peripherals::PIN_15>,
    pub i2c: Peri<'static, peripherals::I2C1>,
    pub interrupt: Peri<'static, peripherals::PIN_13>,
    pub readings_tx: crate::ReadingsSender,
}

#[embassy_executor::task]
pub async fn adc_task(adc_r: AdcResources) {
    defmt::info!("Running ADC process");
    let i2c = i2c::I2c::new_blocking(adc_r.i2c, adc_r.scl, adc_r.sda, i2c::Config::default());
    let mut interrupt = gpio::Input::new(adc_r.interrupt, gpio::Pull::None);

    // Initialize the ADS1115 driver
    let mut adc = ads1x1x::Ads1x1x::new_ads1115(i2c, ads1x1x::TargetAddr::Gnd);
    adc.set_full_scale_range(ads1x1x::FullScaleRange::Within4_096V)
        .unwrap();
    adc.set_data_rate(ads1x1x::DataRate16Bit::Sps860).unwrap();

    let mut adc = match adc.into_continuous() {
        Err(ads1x1x::ModeChangeError::I2C(e, _)) => {
            panic!("{:?}", e);
        }
        Ok(adc) => adc,
    };
    adc.use_alert_rdy_pin_as_ready().unwrap();
    adc.set_comparator_queue(ads1x1x::ComparatorQueue::One)
        .unwrap();

    // Store the ADC readings into arrays
    let mut readings_0 = [0i16; AVG_LENGTH];
    let mut readings_1 = [0i16; AVG_LENGTH];

    loop {
        adc.select_channel(ads1x1x::channel::DifferentialA0A1)
            .unwrap();
        interrupt.wait_for_falling_edge().await;
        adc.read().unwrap();
        for value in readings_0.iter_mut() {
            interrupt.wait_for_falling_edge().await;
            *value = adc.read().unwrap();
        }

        adc.select_channel(ads1x1x::channel::DifferentialA2A3)
            .unwrap();
        interrupt.wait_for_falling_edge().await;
        adc.read().unwrap();
        for value in readings_1.iter_mut() {
            interrupt.wait_for_falling_edge().await;
            *value = adc.read().unwrap();
        }

        let pot = average_readings(&mut readings_0);
        let vref = average_readings(&mut readings_1);

        // Ratiometric: normalize pot against the measured supply voltage to
        // cancel noise common to both channels (e.g. LED-induced supply droop).
        // Result is 0–10000 representing 0–100% of travel.
        let position = if vref != 0 { (pot as i64 * 10_000 / vref as i64) as i32 } else { 0 };

        defmt::info!("pos: {}\tpot: {}\tvref: {}", position, pot, vref);
        // Non-blocking send — drops reading if USB serial task hasn't caught up
        let _ = adc_r.readings_tx.try_send(position);
    }
}

fn average_readings(readings: &mut [i16]) -> i32 {
    let mut avg = 0;

    readings.sort_unstable();

    let window = &readings[AVG_CUTOFF..(AVG_LENGTH - AVG_CUTOFF)];

    for val in window {
        avg += *val as i32;
    }
    avg /= window.len() as i32;

    avg
}
