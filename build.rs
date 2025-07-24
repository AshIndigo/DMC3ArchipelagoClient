use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=./src/data/locations.json");
    println!("cargo:rustc-link-lib=msvcrt");
    // Read the JSON file
    let input_path = Path::new("src/data/locations.json");
    let content = fs::read_to_string(input_path).expect("Unable to read locations.json");

    // Parse JSON
    let data: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");

    // Initial stuff for the rust file
    let mut output = String::from("// Auto-generated constants file\n\n");
    output.push_str("use std::collections::HashMap;\n");
    output.push_str("use crate::constants::ItemEntry;\nuse std::sync::LazyLock;\n\n");

    output.push_str("pub static ITEM_MISSION_MAP: LazyLock<HashMap<&'static str, ItemEntry>> = LazyLock::new(|| {
    HashMap::from([\n");
    for (key, value) in data.as_object().expect("Expected JSON object") {
        let offset = value["offset"].as_u64().unwrap();
        let mission_number = value["mission_number"].as_u64().unwrap();
        let room = value["room_number"].as_u64().unwrap();
        let item_id = value["default_item"].as_u64().unwrap();
        let adjudicator = value["adjudicator"].as_bool().unwrap();
        let x_coord = value["xCoord"].as_u64().unwrap();
        let y_coord = value["yCoord"].as_u64().unwrap();
        let z_coord = value["zCoord"].as_u64().unwrap();

        output.push_str(&format!(
            r#"        ("{}", ItemEntry {{ offset: {}, mission: {}, room_number: {}, item_id: {}, adjudicator: {}, x_coord: {}, y_coord: {}, z_coord: {} }}),"#,
            key, offset, mission_number, room, item_id, adjudicator, x_coord, y_coord, z_coord
        ));
        output.push('\n');
    }
    output.push_str("    ])\
    });\n\n");

/*    output.push_str("pub fn get_locations() -> HashMap<&'static str, ItemEntry> {\n");
    output.push_str("    let mut map = HashMap::new();\n");

    // Convert each entry into a HashMap entry
    for (key, value) in data.as_object().expect("Expected JSON object") {
        let offset = value["offset"].as_u64().unwrap();
        let mission_number = value["mission_number"].as_u64().unwrap();
        let room = value["room_number"].as_u64().unwrap();
        let item_id = value["default_item"].as_u64().unwrap();
        let adjudicator = value["adjudicator"].as_bool().unwrap();

        output.push_str(&format!(
            r#"    map.insert("{}", ItemEntry {{ offset: {}, mission: {}, room_number: {}, item_id: {}, adjudicator: {} }});"#,
            key, offset, mission_number, room, item_id, adjudicator
        ));
        output.push('\n');
    }

    // Return map
    output.push_str("    map\n");
    output.push_str("}\n");*/

    // Write to src folder
    let out_dir = Path::new("src");
    let dest_path = Path::new(&out_dir).join("data/generated_locations.rs");
    fs::write(dest_path, output).expect("Unable to write generated_locations");
}
