#![no_main]
#![no_std]

extern crate alloc;
use core::{cell::RefCell, hint::unreachable_unchecked};
use embedded_alloc::LlffHeap as Heap;
use embedded_common::{
    argb::{self, Colour},
    can::CanDriver,
    clock,
};
use heapless::Vec;

use stm32g4xx_hal::{
    prelude::*, rcc::*, pac,
    gpio::GpioExt,
    pwr::{PwrExt, VoltageScale}, 
    time::{ExtU32, RateExtU32}
};

use cortex_m_rt::entry;

use defmt_rtt as _;
use panic_probe as _;

use canadensis::{
    core::{
        transfer::MessageTransfer, transport::Transport, Priority, SubjectId,
    },
    encoding::Deserialize,
    node::{
        data_types::{GetInfoResponse, Version},
        BasicNode, CoreNode,
    },
    requester::TransferIdFixedMap,
    Node, TransferHandler,
};
use canadensis_can::{CanNodeId, CanReceiver, CanTransmitter, CanTransport, Mtu};

// NOTE: import the data types you use
use canadensis_data_types::{
    // Avoid using primitives like these except for debugging; see the Cyphal spec.
    // Preference uavcan/reg data types
    uavcan::primitive::scalar::natural8_1_0,
};

use canadensis_data_types::uavcan::node::execute_command_1_3::SERVICE as EXECUTE_COMMAND_SERVICE;
use canadensis_data_types::uavcan::node::execute_command_1_3::{
    ExecuteCommandRequest, ExecuteCommandResponse,
};

// Cyphal constants
// NOTE: change the NODE_ID to your node's ID
const NODE_ID: u8 = 6;
const CYPHAL_CONCURRENT_TRANSFERS: usize = 4;
const CYPHAL_NUM_TOPICS: usize = 8;
const CYPHAL_NUM_SERVICES: usize = 8;

// NOTE: these are suggested constants but feel free to adjust
const HEARTBEAT_PERIOD_US: u32 = 1_000_000;
const TELEM_PERIOD_US: u32 = 50_000;
const TID_TIMEOUT_US: u32 = 100_000;

// Cyphal IDs
// NOTE: remove this one and add the ones you use
const LED_SUBJECT: SubjectId = SubjectId::from_truncating(3000);

// ARGB LED constants 
// NOTE: uncomment if you want to use these
// const RED: Colour = Colour { r: 255, g: 0, b: 0 };
// const BLUE: Colour = Colour { r: 0, g: 0, b: 255 };
// const GREEN: Colour = Colour { r: 0, g: 255, b: 0 };
// const MAGENTA: Colour = Colour { r: 255, g: 0, b: 255 };
// const CYAN: Colour = Colour { r: 0, g: 255, b: 255 };
// const YELLOW: Colour = Colour { r: 255, g: 255, b: 0 };
const DEFAULT_BRIGHTNESS: u8 = 15;

// Global allocator -- required by canadensis.
#[global_allocator]
static HEAP: Heap = Heap::empty();

fn initialise_allocator() {
    use core::mem::MaybeUninit;
    // NOTE: You need to calculate HEAP_SIZE such that you don't crash when messages are reassembled
    // You may also want to shrink this if there's not so much CAN traffic
    const HEAP_SIZE: usize = 0x1_0000; // 64 KiB
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }
}

#[entry]
fn main() -> ! {
    initialise_allocator();

    // Initialise peripherals and clock.
    let dp = pac::Peripherals::take().unwrap();
    let cp = pac::CorePeripherals::take().unwrap();
    // This power mode allows us to run at the highest clock frequency
    let pwr = dp.PWR.constrain().vos(VoltageScale::Range1 { enable_boost: true }).freeze();
    let mut rcc = dp.RCC.freeze(
        Config::pll()
            .pll_cfg(PllConfig {
                // NOTE: Make sure your board's external crystal is actually 24 MHz!
                mux: PllSrc::HSE(24.MHz()),
                m: PllMDiv::DIV_2,
                n: PllNMul::MUL_28,
                // Run PLLR and PLLQ at 168 MHz (the maximum)
                r: Some(PllRDiv::DIV_2), // used for SYSCLK
                q: Some(PllQDiv::DIV_2), // used for FDCAN
                p: None,
            })
            .fdcan_src(FdCanClockSource::PLLQ),
            pwr
        );

    defmt::debug!("Initialised memory allocator and configured clock");
    defmt::trace!("Setting up pins...");

    // Set up pins.
    // NOTE: remove ones you don't use
    let gpioa = dp.GPIOA.split(&mut rcc);
    let gpiob = dp.GPIOB.split(&mut rcc);
    let gpioc = dp.GPIOC.split(&mut rcc);
    let gpiod = dp.GPIOD.split(&mut rcc);

    defmt::debug!("Setting up LED driver...");
    let mut argb = argb::Controller::new(
        // NOTE: change these to the pin and USART that the LEDs are on
        dp.USART1,
        gpioc.pc4.into_alternate(),
        DEFAULT_BRIGHTNESS,
        &mut rcc,
    );
    defmt::debug!("Setting up microsecond clock...");
    let clock = clock::MicrosecondClock::new(dp.TIM2, &mut rcc);
    defmt::debug!("Setting up FDCAN driver...");
    let can = CanDriver::new(
        // NOTE: change these to the pins and FDCAN that the FDCAN transceiver is on
        dp.FDCAN1,
        gpioa.pa11.into_alternate(),
        gpioa.pa12.into_alternate(),
        &mut rcc,
    );
    
    defmt::debug!("Setting up core Cyphal node...");
    let id = CanNodeId::from_truncating(NODE_ID);
    let transmitter = CanTransmitter::new(Mtu::CanFd64);
    let receiver = CanReceiver::new(id);
    let core_node: CoreNode<
        _,
        _,
        _,
        TransferIdFixedMap<CanTransport, CYPHAL_CONCURRENT_TRANSFERS>,
        _,
        CYPHAL_NUM_TOPICS,
        CYPHAL_NUM_SERVICES,
    > = CoreNode::new(clock, id, transmitter, receiver, can);

    // Node initialisation is a non-recoverable error and should only happen if we run out of
    // memory or the hardware is completely broken, hence all the unwrapping.
    defmt::debug!("Setting up basic Cyphal node...");
    let mut node = BasicNode::new(
        core_node,
        GetInfoResponse {
            // NOTE: Update these appropriately
            protocol_version: Version { major: 1, minor: 0 },
            hardware_version: Version { major: 0, minor: 1 },
            software_version: Version { major: 0, minor: 1 },
            software_vcs_revision_id: 0,
            unique_id: embedded_common::debug::uuid(),
            name: Vec::from_slice(b"org.bluesat.template.updatethis").unwrap(),
            software_image_crc: Vec::new(),
            certificate_of_authenticity: Vec::new(),
        },
    )
    .unwrap();

    // NOTE: add calls like the following if you want to listen for specific messages
    // node.subscribe_message(
    //     EXAMPLE_SUBSCRIPTION_ID,
    //     size_of::<EXAMPLE_SUBSCRIPTION_DATA_TYPE>(),
    //     MicrosecondDuration32::from_ticks(TID_TIMEOUT_US),
    // )
    // .unwrap();

    // NOTE: add calls like the following to publish specific messages
    defmt::trace!("Starting publication of LED messages...");
    node.start_publishing(LED_SUBJECT, 10.millis(), Priority::Nominal).unwrap();

    // Start the superloop.
    let mut tim_heartbeat = node.clock().now_const();
    let mut tim_telem = node.clock().now_const();
    let mut tim_argb = node.clock().now_const();
  
    // NOTE: You almost certainly want to replace the Subsystem with an actual subsystem
    let subsystem = RefCell::new(Subsystem);
    let mut comms_handler = CommsHandler { subsystem: &subsystem };

    defmt::info!("System initialised. Entering superloop...");
    let mut hue: u16 = 0;
    let mut cycles = 0;
    loop {
        // Handle Cyphal tasks
        node.receive(&mut comms_handler).unwrap();

        // You can set the health of the node to represent the state of the subsystem to be
        // signalled over Cyphal/CAN in the heartbeat messages:
        // node.set_health(...); node.set_status_code(...);
        
        if node
            .clock()
            .advance_if_elapsed(&mut tim_heartbeat, HEARTBEAT_PERIOD_US.micros())
        {
            defmt::debug!("Publishing node heartbeat...");
            node.run_per_second_tasks().unwrap();
        }
        
        if node
                .clock()
                .advance_if_elapsed(&mut tim_telem, TELEM_PERIOD_US.micros())
        {
            defmt::trace!("Publishing LED telemetry...");
            node.publish(
                LED_SUBJECT,
                &natural8_1_0::Natural8 {
                    value: hue as u8
                },
            )
            .unwrap();
        }

        // Handle subsystem tasks (ARGB LEDs)
        // If you write cooler LED pattern code than this hit me up. turtle@turtle.business
        fn hue2component(h: u16) -> u16 {
            const PHASE: u16 = 180;
            if h < PHASE / 2 {
                h * 2
            } else if h < PHASE {
                PHASE - 2 * (h - PHASE / 2)
            } else {
                0
            }
        }

        fn hue2color(h: u16) -> Colour {
            let c1 = hue2component(h) as u8;
            let c2 = hue2component((h + 85) % u8::MAX as u16) as u8;
            let c3 = hue2component((h + 170) % u8::MAX as u16) as u8;
            Colour { r: c1, g: c2, b: c3 }
        }

        if node
            .clock()
            .advance_if_elapsed(&mut tim_argb, 6.millis())
        {
            hue = (hue + 1) % u8::MAX as u16;

            let col1 = hue2color(hue);
            let col2 = hue2color((hue + 16) % u8::MAX as u16);
            let col3 = hue2color((hue + 32) % u8::MAX as u16);
            let col4 = hue2color((hue + 48) % u8::MAX as u16);
            let col5 = hue2color((hue + 64) % u8::MAX as u16);

            defmt::trace!("Refreshing LEDs...");
            argb.display(&[col1, col2, col3, col4, col5]);
            
            if hue == 0 {
                cycles += 1;
                if cycles == 1 {
                    defmt::println!("LEDs have done {} full colour cycle", cycles);
                } else {
                    defmt::println!("LEDs have done {} full colour cycles", cycles);
                }
            }
        }
    }
}

// NOTE: replace with your actual subsystem
struct Subsystem;

// NOTE: update this
struct CommsHandler<'a> {
    subsystem: &'a RefCell<Subsystem>
}

impl<T: Transport> TransferHandler<T> for CommsHandler<'_> {
    fn handle_message<N: Node<Transport = T>>(
        &mut self,
        _node: &mut N,
        transfer: &MessageTransfer<alloc::vec::Vec<u8>, T>,
    ) -> bool {
        // NOTE: add your message handler here
        defmt::println!("Received a message with subject ID {}", u16::from(transfer.header.subject));
        let mut subsystem = self.subsystem.borrow_mut();
        // let msg = DATA_TYPE::deserialize_from_bytes(&transfer.payload);
        // TODO: do something with the subsystem
        true
    }

    fn handle_request<N: Node<Transport = T>>(
        &mut self,
        node: &mut N,
        token: canadensis::ResponseToken<T>,
        transfer: &canadensis::core::transfer::ServiceTransfer<alloc::vec::Vec<u8>, T>,
    ) -> bool {
        if transfer.header.service != EXECUTE_COMMAND_SERVICE {
            return false;
        }

        let req =
            ExecuteCommandRequest::deserialize_from_bytes(transfer.payload.as_slice()).unwrap();
        match req.command {
            // handle COMMAND_RESTART
            ExecuteCommandRequest::COMMAND_RESTART => {
                unsafe {
                    stm32g4xx_hal::stm32g4::stm32g474::CorePeripherals::steal()
                        .SCB
                        .aircr
                        .write(0x05FA_0004);
                };
                // SAFETY: The above operation will instantly reset the  MCU
                unsafe { unreachable_unchecked() }
            }
            // NOTE: add any other request handlers here
            _ => {
                node.send_response(
                    token,
                    1000.millis(),
                    &ExecuteCommandResponse {
                        status: ExecuteCommandResponse::STATUS_BAD_COMMAND,
                        output: Vec::new(),
                    },
                )
                .unwrap();
            }
        }

        true
    }
}
