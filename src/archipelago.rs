use crate::check_handler::{Location, LocationType, TX_LOCATION};
use crate::constants::{MISSION_ITEM_MAP, REMOTE_ID};
use crate::game_manager::{get_mission, ArchipelagoData, Style, ARCHIPELAGO_DATA};
use crate::mapping::{DeathlinkSetting, Goal, Mapping, MAPPING};
use crate::ui::font_handler::{WHITE, YELLOW};
use crate::ui::overlay::{MessageSegment, MessageType, OverlayMessage};
use crate::ui::{overlay, text_handler};
use crate::{
    constants, game_manager, hook, item_sync, location_handler, mapping, skill_manager, utilities,
};

use randomizer_utilities::ui_utilities::Status;

use crate::item_sync::CURRENT_INDEX;
use archipelago_rs::{
    AsItemId, Client, ClientStatus, Connection, ConnectionOptions, ConnectionState, CreateAsHint,
    DeathLinkOptions, Event, ItemHandling, ReceivedItem,
};
use randomizer_utilities::archipelago_utilities::{handle_print, DeathLinkData};
use randomizer_utilities::{archipelago_utilities, setup_channel_pair};
use serde_json::Value;
use std::error::Error;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::OnceLock;
use std::time::Duration;

pub(crate) static CONNECTION_STATUS: AtomicIsize = AtomicIsize::new(0);
pub static TX_DEATHLINK: OnceLock<Sender<DeathLinkData>> = OnceLock::new();

pub struct ArchipelagoCore {
    pub connection: Connection<Value>,
    hooks_installed: bool,
    hooks_enabled: bool,

    location_receiver: Receiver<Location>,
    deathlink_receiver: Receiver<DeathLinkData>,
    //bank_receiver: Receiver<(&'static str, i32)>,
}

impl ArchipelagoCore {
    pub fn new(url: String, game_name: String) -> anyhow::Result<Self> {
        Ok(Self {
            connection: Connection::new(
                url,
                game_name,
                "",
                ConnectionOptions::new().receive_items(ItemHandling::OtherWorlds {
                    own_world: true,
                    starting_inventory: true,
                }),
            ),
            hooks_installed: false,
            hooks_enabled: false,
            location_receiver: setup_channel_pair(&TX_LOCATION),
            deathlink_receiver: setup_channel_pair(&TX_DEATHLINK),
            //bank_receiver: setup_channel_pair(&TX_BANK_MESSAGE),
        })
    }

    pub fn update(&mut self) -> Result<(), Box<dyn Error>> {
        for event in self.connection.update() {
            match event {
                Event::Connected => {
                    log::info!("Connected!");
                    if !self.hooks_installed {
                        // Hooks needed to modify the game
                        unsafe {
                            match hook::create_hooks() {
                                Ok(_) => {
                                    log::debug!("Created DMC3 Hooks");
                                }
                                Err(err) => {
                                    log::error!("Failed to create hooks: {:?}", err);
                                }
                            }
                        }
                        self.hooks_installed = true;
                    }
                    if !self.hooks_enabled {
                        hook::enable_hooks();
                        self.hooks_enabled = true;
                    }
                    run_setup(self.connection.client_mut().unwrap())?;
                }
                Event::Updated(_) => {}
                Event::Print(print) => {
                    let str = handle_print(print);
                    log::info!("Print from server: {}", str);
                }
                Event::ReceivedItems(idx) => {
                    handle_received_items_packet(
                        idx,
                        self.connection.client_mut().unwrap().received_items(),
                    )?;
                }
                Event::Error(err) => log::error!("{}", err),
                Event::Bounce { .. } => {}
                Event::DeathLink {
                    games: _,
                    slots: _,
                    tags: _,
                    time: _,
                    cause,
                    source,
                } => {
                    overlay::add_message(OverlayMessage::new(
                        vec![MessageSegment::new(
                            format!("{}: {}", source, cause.unwrap_or_default()),
                            WHITE,
                        )],
                        Duration::from_secs(3),
                        // TODO May want to adjust position, currently added to the 'notification list' so it's in the upper right queue
                        0.0,
                        0.0,
                        MessageType::Notification,
                    ));
                    match MAPPING.read()?.as_ref().unwrap().death_link {
                        DeathlinkSetting::DeathLink => {
                            game_manager::kill_dante();
                        }
                        DeathlinkSetting::HurtLink => {
                            game_manager::hurt_dante();
                        }
                        DeathlinkSetting::Off => {}
                    }
                }
                Event::KeyChanged {
                    key: _,
                    old_value: _,
                    new_value: _,
                    player: _,
                } => {}
            }
        }
        match self.connection.state() {
            ConnectionState::Connecting(_) => {}
            ConnectionState::Connected(_) => {
                CONNECTION_STATUS.store(Status::Connected.into(), Ordering::SeqCst);
            }
            ConnectionState::Disconnected(state) => {
                CONNECTION_STATUS.store(Status::Disconnected.into(), Ordering::SeqCst);
                disconnect();
                return Err(format!("Disconnected from server: {:?}", state).into());
            }
        }
        self.handle_channels()?;
        Ok(())
    }

    pub fn handle_channels(&mut self) -> Result<(), Box<dyn Error>> {
        match self.location_receiver.try_recv() {
            Ok(location) => {
                handle_item_receive(self.connection.client_mut().unwrap(), location)?;
            }
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    return Err("Disconnected from location receiver".into());
                }
            }
        }

        match self.deathlink_receiver.try_recv() {
            Ok(dl_data) => self
                .connection
                .client_mut()
                .unwrap()
                .death_link(DeathLinkOptions::new().cause(dl_data.cause))?,
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    return Err("Disconnected from DeathLink receiver".into());
                }
            }
        }
        Ok(())
    }
}

/// This is run when a there is a valid connection to a room.
pub fn run_setup(client: &mut Client) -> Result<(), Box<dyn Error>> {
    log::info!("Running setup");
    hook::rewrite_mode_table();

    match mapping::parse_slot_data(client) {
        Ok(_) => {
            log::info!("Successfully parsed mapping information");
            const DEBUG: bool = false;
            if DEBUG {
                log::debug!("Mapping data: {:#?}", MAPPING.read()?);
            }
        }
        Err(err) => {
            return Err(
                format!("Failed to load mappings from slot data, aborting: {}", err).into(),
            );
        }
    }
    mapping::run_scouts_for_mission(client, constants::NO_MISSION, CreateAsHint::New);
    mapping::run_scouts_for_secret_mission(client);
    Ok(())
}

fn disconnect() {
    // TODO I want this to actually be useful. Need to make sure its called when I disconnect on the proxy
    log::info!("Disconnecting and restoring game");
    match hook::disable_hooks() {
        Ok(_) => {
            log::debug!("Disabled hooks");
        }
        Err(e) => {
            log::error!("Failed to disable hooks: {:?}", e);
        }
    }
    MAPPING.write().unwrap().take(); // Clear mappings
    *ARCHIPELAGO_DATA.write().unwrap() = ArchipelagoData::default(); // Reset Data (Probably not needed)
    hook::restore_mode_table();
    log::info!("Game restored to default state");
}

fn handle_item_receive(client: &mut Client, received_item: Location) -> Result<(), Box<dyn Error>> {
    // See if there's an item!
    log::info!("Processing item: {}", received_item);
    if let Some(mapping_data) = MAPPING.read()?.as_ref() {
        if received_item.location_type == LocationType::Standard && received_item.item_id <= 0x39 {
            crate::check_handler::take_away_received_item(received_item.item_id);
        }
        let location_key = location_handler::get_location_name_by_data(&received_item)?;
        // Then see if the item picked up matches the specified in the map
        match mapping::CACHED_LOCATIONS.read()?.get(location_key) {
            Some(located_item) => {
                location_handler::edit_end_event(location_key); // Needed so a mission will end properly after picking up its trigger.
                text_handler::replace_unused_with_text(archipelago_utilities::get_description(
                    located_item,
                ));
                text_handler::CANCEL_TEXT.store(true, Ordering::SeqCst);
                if let Err(arch_err) = client.mark_checked(vec![located_item.location()]) {
                    log::error!("Failed to check location: {}", arch_err);
                    let _index = item_sync::get_sync_file_key(
                        client.seed_name(),
                        client.this_player().name().parse()?,
                    );
                    // TODO Offline Checks
                    //item_sync::add_offline_check(located_item.location().id(), index)?;
                }
                let name = located_item.item().name();
                let in_game_id = if located_item.sender() == located_item.receiver() {
                    located_item.item().as_item_id() as u32
                } else {
                    *REMOTE_ID
                };
                if let Ok(mut archipelago_data) = ARCHIPELAGO_DATA.write()
                    && in_game_id > 0x14
                    && in_game_id != *REMOTE_ID
                {
                    archipelago_data.add_item(located_item.item().name().to_string());
                }

                log::info!(
                    "Location check successful: {}, Item: {}",
                    location_key,
                    name
                );
            }
            None => Err(anyhow::anyhow!("Location not found: {}", location_key))?,
        }
        // Add to checked locations
        if has_reached_goal(client, &mapping_data) {
            client.set_status(ClientStatus::Goal)?
        }
    }

    Ok(())
}

fn has_reached_goal(client: &mut Client, mapping: &&Mapping) -> bool {
    let mut chk = client.checked_locations();
    match mapping.goal {
        Goal::Standard => chk.any(|loc| loc.name() == "Mission #20 Complete"),
        Goal::All => {
            for i in 1..20 {
                // If we are missing a mission complete check then we cannot goal
                if !chk.any(|loc| loc.name() == format!("Mission #{} Complete", i).as_str()) {
                    return false;
                }
            }
            // If we have them all, goal
            true
        }
        Goal::RandomOrder => {
            if let Some(order) = &mapping.mission_order {
                return chk
                    .any(|loc| loc.name() == format!("Mission #{} Complete", order[19]).as_str());
            }
            false
        }
    }
}

pub fn handle_received_items_packet(
    index: usize,
    received_items: &[ReceivedItem],
) -> Result<(), Box<dyn Error>> {
    if game_manager::session_is_valid() {
        if index == 0 {
            // If 0 reset stored data
            *ARCHIPELAGO_DATA.write()? = ArchipelagoData::default();
        }
        match ARCHIPELAGO_DATA.write() {
            Ok(mut data) => {
                for item in received_items
                    .iter()
                    .filter(|item| item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize)
                {
                    // Display overlay text if we're not at the main menu
                    if !utilities::is_on_main_menu() {
                        let rec_msg: Vec<MessageSegment> = vec![
                            MessageSegment::new("Received ".to_string(), WHITE),
                            MessageSegment::new(
                                item.item().name().to_string(),
                                overlay::get_color_for_item(item.as_ref()),
                            ),
                            MessageSegment::new(" from ".to_string(), WHITE),
                            MessageSegment::new(item.sender().name().parse()?, YELLOW),
                        ];
                        overlay::add_message(OverlayMessage::new(
                            rec_msg,
                            Duration::from_secs(3),
                            0.0,
                            0.0,
                            MessageType::Notification,
                        ));
                    }

                    match item.item().as_item_id() {
                        0x07 => {
                            data.add_blue_orb();
                            game_manager::give_hp(constants::ONE_ORB);
                        }
                        0x08 => {
                            data.add_purple_orb();
                            game_manager::give_magic(constants::ONE_ORB, &data);
                        }
                        0x10..0x14 => {
                            game_manager::add_consumable(item.item());
                        }
                        0x19 => {
                            // Awakened Rebellion
                            data.add_dt();
                            game_manager::give_magic(constants::ONE_ORB * 3.0, &data);
                        }
                        0x24..0x3A => {
                            // For key items
                            log::debug!("Setting newly acquired key items");
                            match MISSION_ITEM_MAP.get(&(get_mission())) {
                                None => {} // No items for the mission
                                Some(item_list) => {
                                    if item_list.contains(&&*item.item().name()) {
                                        game_manager::set_item(&item.item().name(), true, true);
                                    }
                                }
                            }
                        }
                        0x3A..0x53 => {
                            // For skills
                            if let Some(mapping) = MAPPING.read()?.as_ref()
                                && mapping.randomize_skills
                            {
                                skill_manager::add_skill(item.item().id() as usize, &mut data);
                                skill_manager::set_skills(&data); // Hacky...
                            }
                        }
                        0x53 => {
                            // Ebony & Ivory
                            data.add_gun_level(0);
                        }
                        0x54 => {
                            // Shotgun
                            data.add_gun_level(1);
                        }
                        0x55 => {
                            // Artemis
                            data.add_gun_level(2);
                        }
                        0x56 => {
                            // Spiral
                            data.add_gun_level(3);
                        }
                        0x57 => {
                            // Kalina Ann
                            data.add_gun_level(4);
                        }
                        0x60 => {
                            data.add_style_level(Style::Trickster);
                            game_manager::apply_style_levels(Style::Trickster)
                        }
                        0x61 => {
                            data.add_style_level(Style::Swordmaster);
                            game_manager::apply_style_levels(Style::Swordmaster)
                        }
                        0x62 => {
                            data.add_style_level(Style::Gunslinger);
                            game_manager::apply_style_levels(Style::Gunslinger)
                        }
                        0x63 => {
                            data.add_style_level(Style::Royalguard);
                            game_manager::apply_style_levels(Style::Royalguard)
                        }
                        // Weapons
                        0x16..0x19 => {
                            // R, C, A&I
                        }
                        0x1A..0x22 => {
                            // Rest of weapons
                        }
                        _ => {
                            log::warn!(
                                "Unhandled item ID: {} ({:#X})",
                                item.item().name(),
                                item.item().id()
                            )
                        }
                    }

                    data.add_item(item.item().name().into());
                    CURRENT_INDEX.store(item.index() as i64, Ordering::SeqCst);
                }
            }
            Err(err) => {
                log::error!("Failed to write archipelago data: {}", err);
            }
        }
    }
    Ok(())
}
