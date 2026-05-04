use crate::constants::{MISSION_ITEM_MAP, REMOTE_ID, Style};
use crate::game_manager::{ARCHIPELAGO_DATA, ArchipelagoData, get_mission, set_weapons_in_inv};
use crate::hooks::check_handler::{Location, LocationType, TX_LOCATION, take_away_received_item};
use crate::mapping::{
    AutoHint, DeathlinkSetting, Goal, MAPPING, ModMode, ModModeData, OVERLAY_INFO, OverlayInfo,
    get_adjudicators, get_mission_completes, get_secret_missions,
};
use crate::ui::text_handler;
use crate::{constants, game_manager, hint_game, location_handler, skill_manager, utilities};
use randomizer_utilities::ui::font_handler::{WHITE, YELLOW};
use std::env;

use crate::data::game_structs::{GameData, SessionData};
use crate::data::generated_locations;
use crate::hint_game::TX_HINT;
use crate::hooks::{check_handler, hook};
use archipelago_rs::{
    AsItemId, Client, ClientStatus, Connection, ConnectionOptions, ConnectionState, CreateAsHint,
    DeathLinkOptions, Event, ItemHandling,
};
use randomizer_utilities::archipelago_utilities::{DeathLinkData, handle_print};
use randomizer_utilities::item_sync::CURRENT_INDEX;
use randomizer_utilities::ui::overlay_messages;
use randomizer_utilities::ui::overlay_messages::{MessageSegment, MessageType, OverlayMessage};
use randomizer_utilities::{archipelago_utilities, item_sync, setup_channel_pair};
use std::error::Error;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::time::Duration;

pub(crate) static CONNECTED: AtomicBool = AtomicBool::new(false);
pub static TX_DEATHLINK: OnceLock<Sender<DeathLinkData>> = OnceLock::new();

pub struct ArchipelagoCore {
    pub connection: Connection<ModModeData>,
    hooks_installed: bool,
    hooks_enabled: bool,

    hint_hooks_installed: bool,
    hint_hooks_enabled: bool,

    location_receiver: Receiver<Location>,
    deathlink_receiver: Receiver<DeathLinkData>,
    hint_receiver: Receiver<Vec<i64>>,
}

impl ArchipelagoCore {
    pub fn new(url: String, game_name: String) -> anyhow::Result<Self> {
        Ok(Self {
            connection: Connection::new(
                url,
                "",
                Some(game_name),
                ConnectionOptions::new().receive_items(ItemHandling::OtherWorlds {
                    own_world: true,
                    starting_inventory: true,
                }),
            ),
            hooks_installed: false,
            hooks_enabled: false,
            hint_hooks_installed: false,
            hint_hooks_enabled: false,
            location_receiver: setup_channel_pair(&TX_LOCATION),
            deathlink_receiver: setup_channel_pair(&TX_DEATHLINK),
            hint_receiver: setup_channel_pair(&TX_HINT),
        })
    }

    pub fn update(&mut self) -> Result<(), Box<dyn Error>> {
        for event in self.connection.update() {
            match event {
                Event::Connected => {
                    log::info!("Connected!");
                    log::debug!("Mod version: {}", env!("CARGO_PKG_VERSION"));
                    let mut overlay_info = OVERLAY_INFO.write()?;
                    match self.connection.client().unwrap().slot_data() {
                        ModModeData::HintGame(mapping) => {
                            log::info!("Running in hint game mode");
                            overlay_info.client_version = mapping.client_version;
                            overlay_info.generated_version = None;
                            overlay_info.mode = ModMode::HintGame;
                            if !self.hint_hooks_installed {
                                unsafe {
                                    match hint_game::create_hint_hooks() {
                                        Ok(_) => {
                                            log::debug!("Created DMC3 Hint Hooks");
                                            self.hint_hooks_installed = true;
                                        }
                                        Err(err) => {
                                            log::error!("Failed to create hint hooks: {:?}", err);
                                        }
                                    }
                                }
                            }
                            if self.hint_hooks_installed && !self.hint_hooks_enabled {
                                unsafe {
                                    hint_game::enable_hint_hooks();
                                    self.hint_hooks_enabled = true;
                                }
                            }
                            hint_game::FLOORS_PER_HINT
                                .store(mapping.floors_per_hint, Ordering::Relaxed);
                        }
                        ModModeData::Normal(mapping) => {
                            log::info!("Running in randomizer mode");
                            overlay_info.generated_version = mapping.generated_version;
                            overlay_info.client_version = mapping.client_version;
                            overlay_info.mode = ModMode::Normal;
                            MAPPING.write()?.replace(mapping.clone());
                            item_sync::send_offline_checks(self.connection.client_mut().unwrap())?;
                            if !self.hooks_installed {
                                // Hooks needed to modify the game
                                unsafe {
                                    match hook::create_hooks() {
                                        Ok(_) => {
                                            log::debug!("Created DMC3 Hooks");
                                            self.hooks_installed = true;
                                        }
                                        Err(err) => {
                                            log::error!("Failed to create hooks: {:?}", err);
                                        }
                                    }
                                }
                            }
                            if self.hooks_installed && !self.hooks_enabled {
                                hook::enable_hooks();
                                self.hooks_enabled = true;
                            }
                            run_setup(self.connection.client_mut().unwrap())?;
                        }
                    }
                    // Print out version info
                    log::debug!(
                        "Client version: {}",
                        if let Some(cv) = overlay_info.client_version {
                            cv.to_string()
                        } else {
                            "Unknown".to_string()
                        }
                    );
                    if overlay_info.mode == ModMode::Normal {
                        log::debug!(
                            "Generated version: {}",
                            if let Some(gv) = overlay_info.generated_version {
                                gv.to_string()
                            } else {
                                "Unknown".to_string()
                            }
                        );
                    }
                }
                Event::Updated(_) => {}
                Event::Print(print) => {
                    let str = handle_print(print);
                    log::info!("Print from server: {}", str);
                }
                Event::ReceivedItems(idx) => {
                    handle_received_items_packet(idx, self.connection.client_mut().unwrap())?;
                }
                Event::Error(err) => log::error!("{}", err),
                Event::Bounce {
                    games: _,
                    slots: _,
                    tags: _,
                    data,
                } => {
                    if let ModModeData::HintGame(_) =
                        self.connection.client_mut().unwrap().slot_data()
                        && let Some(data) = data
                    {
                        let val = data["floors_per_hint"]
                            .as_str()
                            .and_then(|s| s.parse::<u16>().ok())
                            .unwrap_or(80);
                        hint_game::FLOORS_PER_HINT.store(val, Ordering::SeqCst);
                    }
                }
                Event::DeathLink { cause, source, .. } => {
                    overlay_messages::add_message(OverlayMessage::new(
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
                    if let ModModeData::Normal(data) = self.connection.client().unwrap().slot_data()
                    {
                        match data.death_link {
                            DeathlinkSetting::DeathLink => {
                                game_manager::kill_dante();
                            }
                            DeathlinkSetting::HurtLink => {
                                game_manager::hurt_dante();
                            }
                            DeathlinkSetting::Off => {}
                        }
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
                CONNECTED.store(true, Ordering::SeqCst);
            }
            ConnectionState::Disconnected(state) => {
                CONNECTED.store(false, Ordering::SeqCst);
                *OVERLAY_INFO.write()? = OverlayInfo::default();
                disconnect(&mut self.hooks_enabled, &mut self.hint_hooks_enabled);
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
            Ok(dl_data) => {
                if let Some(client) = self.connection.client_mut() {
                    client.death_link(DeathLinkOptions::new().cause(dl_data.cause))?
                }
            }
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    return Err("Disconnected from DeathLink receiver".into());
                }
            }
        }

        match self.hint_receiver.try_recv() {
            Ok(hint_data) => {
                let client = self.connection.client_mut().unwrap();
                client.create_hints(hint_data)?
            }
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    return Err("Disconnected from hint receiver".into());
                }
            }
        }
        Ok(())
    }
}

/// This is run when a there is a valid connection to a room.
pub fn run_setup(client: &mut Client<ModModeData>) -> Result<(), Box<dyn Error>> {
    log::info!("Running setup");
    hook::rewrite_mode_table();
    // Secret Mission Scouts
    archipelago_utilities::run_scouts(
        client.scout_locations(get_secret_missions(client), CreateAsHint::No),
    );
    // Adjudicator Scouts
    archipelago_utilities::run_scouts(
        client.scout_locations(get_adjudicators(client), CreateAsHint::No),
    );
    // Mission Completion Scouts
    archipelago_utilities::run_scouts(
        client.scout_locations(get_mission_completes(client), CreateAsHint::No),
    );

    // Handle auto hinting
    if let ModModeData::Normal(mapping) = client.slot_data() {
        let mut locations_to_scout: Vec<i64> = vec![];
        if mapping.shop_orb_checks {
            let orb_checks: Vec<_> = generated_locations::ITEM_MISSION_MAP
                .iter()
                .filter(|(k, _)| {
                    k.contains("Purchase Purple Orb") || k.contains("Purchase Blue Orb")
                })
                .map(|(&k, _)| client.this_game().location_by_name(k).unwrap().id())
                .collect();
            locations_to_scout.extend(&orb_checks);
        }
        if mapping.shop_gun_checks {
            let gun_checks = generated_locations::ITEM_MISSION_MAP
                .iter()
                .filter(|(k, _)| {
                    constants::GUN_NAMES
                        .iter()
                        .any(|gun_name| k.starts_with("Purchase ") && k.contains(gun_name))
                })
                .map(|(&k, _)| client.this_game().location_by_name(k).unwrap().id())
                .collect::<Vec<i64>>();
            log::debug!("Gun checks: {:?}", gun_checks);
            locations_to_scout.extend(&gun_checks);
        }
        // if AutoHint::All == mapping.auto_skill_hints {
        //     // locations_to_hint.extend(
        //     //     generated_locations::ITEM_MISSION_MAP
        //     //         .iter()
        //     //         .filter(|(k, _)| k.contains("Purple Orb") || k.contains("Blue Orb"))
        //     //         .map(|(k, _)| &client.this_game().location_by_name(*k).unwrap().id())
        //     //         .collect::<Vec<i64>>())
        // }
        if !locations_to_scout.is_empty() {
            archipelago_utilities::run_scouts(
                client.scout_locations(locations_to_scout, CreateAsHint::No),
            );
        }
    }
    Ok(())
}

fn disconnect(hooks_enabled: &mut bool, hint_hooks_enabled: &mut bool) {
    log::info!("Disconnecting and restoring game");
    if *hooks_enabled {
        match hook::disable_hooks() {
            Ok(_) => {
                log::debug!("Disabled hooks");
                *hooks_enabled = false;
            }
            Err(e) => {
                log::error!("Failed to disable hooks: {:?}", e);
            }
        }
    }

    if *hint_hooks_enabled {
        match hint_game::disable_hint_hooks() {
            Ok(_) => {
                log::debug!("Disabled hint hooks");
                *hint_hooks_enabled = false;
            }
            Err(e) => {
                log::error!("Failed to disable hint hooks: {:?}", e);
            }
        }
    }
    MAPPING.write().unwrap().take(); // Clear mappings
    *ARCHIPELAGO_DATA.write().unwrap() = ArchipelagoData::default(); // Reset Data (Probably not needed)
    hook::restore_mode_table();
    log::info!("Game restored to default state");
}

fn handle_item_receive(
    client: &mut Client<ModModeData>,
    received_item: Location,
) -> Result<(), Box<dyn Error>> {
    // See if there's an item!
    log::info!("Processing item: {}", received_item);
    if received_item.location_type == LocationType::Standard
        && received_item.item_id <= 0x39
        && check_handler::should_snatch_item(received_item.item_id)
    {
        take_away_received_item(received_item.item_id);
    }
    let location_key = location_handler::get_location_name_by_data(&received_item)?;
    // Then see if the item picked up matches the specified in the map
    match archipelago_utilities::CACHED_LOCATIONS
        .read()?
        .get(location_key)
    {
        Some(located_item) => {
            if received_item.to_display {
                let rec_msg: Vec<MessageSegment> = vec![
                    MessageSegment::new("Sent ".to_string(), WHITE),
                    MessageSegment::new(
                        located_item.item().name().to_string(),
                        overlay_messages::get_color_for_item(located_item),
                    ),
                    MessageSegment::new(" to ".to_string(), WHITE),
                    MessageSegment::new(located_item.receiver().alias().parse()?, YELLOW),
                ];
                overlay_messages::add_message(OverlayMessage::new(
                    rec_msg,
                    Duration::from_secs(3),
                    0.0,
                    0.0,
                    MessageType::Notification,
                ));
            }
            location_handler::edit_end_event(location_key); // Needed so a mission will end properly after picking up its trigger.
            text_handler::replace_unused_with_text(archipelago_utilities::get_description(
                located_item,
            ));
            text_handler::CANCEL_TEXT.store(true, Ordering::SeqCst);
            if let Err(arch_err) = client.mark_checked(vec![located_item.location()]) {
                log::error!("Failed to check location: {}", arch_err);
                item_sync::add_offline_check(located_item.location().id());
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
    if has_reached_goal(client) {
        client.set_status(ClientStatus::Goal)?
    }
    Ok(())
}

fn has_reached_goal(client: &mut Client<ModModeData>) -> bool {
    let mut chk = client.checked_locations();
    match client.slot_data() {
        ModModeData::HintGame(_) => {
            log::error!("Trying to check for goal in HintGame mode");
            false
        }
        ModModeData::Normal(mapping) => {
            match mapping.goal {
                Goal::Standard => chk.any(|loc| loc.name() == "Mission #20 Complete"),
                Goal::All => {
                    for i in 1..20 {
                        // If we are missing a mission complete check then we cannot goal
                        if !chk.any(|loc| loc.name() == format!("Mission #{} Complete", i).as_str())
                        {
                            return false;
                        }
                    }
                    // If we have them all, goal
                    true
                }
                Goal::RandomOrder => {
                    if let Some(order) = &mapping.mission_order {
                        return chk.any(|loc| {
                            loc.name() == format!("Mission #{} Complete", order[19]).as_str()
                        });
                    }
                    false
                }
            }
        }
    }
}

pub fn handle_received_items_packet(
    index: usize,
    client: &mut Client<ModModeData>,
) -> Result<(), Box<dyn Error>> {
    if SessionData::is_valid() {
        if index == 0 {
            // If 0 reset stored data
            *ARCHIPELAGO_DATA.write()? = ArchipelagoData::default();
        }
        match ARCHIPELAGO_DATA.write() {
            Ok(mut data) => {
                // Not particularly proud of this
                // TODO Maybe try to save+restore ARCHIPELAGO_DATA's state?
                data.blue_orbs = 0;
                data.purple_orbs = 0;
                data.reset_gun_levels();
                data.reset_style_levels();

                for item in client.received_items().iter() {
                    // Display overlay text if we're not at the main menu
                    if !utilities::is_on_main_menu()
                        && item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize
                    {
                        let rec_msg: Vec<MessageSegment> = vec![
                            MessageSegment::new("Received ".to_string(), WHITE),
                            MessageSegment::new(
                                item.item().name().to_string(),
                                overlay_messages::get_color_for_item(item.as_ref()),
                            ),
                            MessageSegment::new(" from ".to_string(), WHITE),
                            MessageSegment::new(item.sender().alias().parse()?, YELLOW),
                        ];
                        overlay_messages::add_message(OverlayMessage::new(
                            rec_msg,
                            Duration::from_secs(3),
                            0.0,
                            0.0,
                            MessageType::Notification,
                        ));
                    }
                    data.add_item(item.item().name().into());
                    match item.item().as_item_id() {
                        0x01..0x04 => {
                            if item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize {
                                let orbs = match item.item().as_item_id() {
                                    1 => 1000,
                                    2 => 2500,
                                    3 => 5000,
                                    _ => unreachable!(),
                                };
                                game_manager::give_red_orbs(orbs);
                            }
                        }
                        0x07 => {
                            data.add_blue_orb();
                            if item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize {
                                game_manager::give_hp(constants::ONE_ORB, &data);
                            }
                        }
                        0x08 => {
                            data.add_purple_orb();
                            if item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize {
                                game_manager::give_magic(constants::ONE_ORB, &data);
                            }
                        }
                        0x10..0x14 => {
                            // Don't add duplicate consumables
                            if item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize {
                                game_manager::add_consumable(item.item());
                            }
                        }
                        0x19 => {
                            // Awakened Rebellion
                            data.add_dt();
                            if item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize {
                                game_manager::give_magic(constants::ONE_ORB * 3.0, &data);
                            }
                        }
                        0x22..0x24 => {
                            // Quicksilver and Doppel
                        }
                        0x24..0x3A => {
                            // For key items
                            if let Some(item_list) = MISSION_ITEM_MAP.get(&(get_mission()))
                                && item_list.contains(&&*item.item().name())
                            {
                                log::debug!("Adding key item: {}", item.item().name());
                                game_manager::set_item(&item.item().name(), true, true);
                            }
                        }

                        0x3A..0x53 => {
                            // For skills
                            if let ModModeData::Normal(mapping) = client.slot_data()
                                && mapping.randomize_skills
                            {
                                skill_manager::add_skill(item.item().id() as usize, &mut data);
                                skill_manager::set_skills(&data); // Hacky...
                            }
                        }
                        0x53..0x58 => {
                            // Gun Levels
                            data.add_gun_level((item.item().id() - 0x53) as usize);
                            game_manager::set_gun_levels(&data);
                        }
                        0x60..0x64 => {
                            // Style Handling
                            let style = match item.item().id() {
                                0x60 => Style::Trickster,
                                0x61 => Style::Swordmaster,
                                0x62 => Style::Gunslinger,
                                0x63 => Style::Royalguard,
                                _ => unreachable!(),
                            };
                            data.add_style_level(style);
                            if item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize {
                                game_manager::apply_style_levels(style);
                            }
                        }
                        // Weapons
                        0x16..=0x18 => {
                            // Rebellion, Cerberus, Agni and Rudra
                            set_weapons_in_inv(&data);
                        }
                        0x1A..=0x1B => {
                            // Nevan, Beowulf
                            set_weapons_in_inv(&data);
                        }
                        0x1C..=0x21 => {
                            // All guns
                            if let ModModeData::Normal(mapping) = client.slot_data()
                                && mapping.shop_gun_checks
                                && mapping.auto_gun_hints == AutoHint::Obtained
                            {
                                let ids: Vec<_> = [2, 3]
                                    .into_iter()
                                    .map(|lvl| {
                                        client
                                            .this_game()
                                            .location_by_name(format!(
                                                "Purchase {} Level {}",
                                                item.item().name(),
                                                lvl
                                            ))
                                            .unwrap()
                                            .id()
                                    })
                                    .collect();

                                TX_HINT.get().unwrap().send(ids)?;
                            }
                            set_weapons_in_inv(&data);
                        }
                        _ => {
                            log::warn!(
                                "Unhandled item ID: {} ({:#X})",
                                item.item().name(),
                                item.item().id()
                            )
                        }
                    }

                    if item.index() >= CURRENT_INDEX.load(Ordering::SeqCst) as usize {
                        CURRENT_INDEX.store((item.index() + 1) as i64, Ordering::SeqCst);
                    }
                }
            }
            Err(err) => {
                log::error!("Failed to write archipelago data: {}", err);
            }
        }
    }
    Ok(())
}
