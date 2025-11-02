// use protocol::keyframe::Keyframe;
// use serde::{Deserialize, Serialize};
//
// pub(crate) fn prompt_and_load_keyframes() -> Option<Vec<Keyframe>> {
//     let file = rfd::FileDialog::new()
//         .add_filter("JSON files", &["json"])
//         .pick_file()?;
//
//     let data = std::fs::read_to_string(file).ok()?;
//     let data = serde_json::from_str::<SaveFormat>(&data).ok()?;
//     Some(data.to_current())
// }
//
// pub(crate) fn prompt_and_save_keyframes(keyframes: &[Keyframe]) -> std::io::Result<()> {
//     if let Some(file) = rfd::FileDialog::new()
//         .add_filter("JSON files", &["json"])
//         .set_file_name("timeline.json")
//         .save_file()
//     {
//         let json = serde_json::to_string_pretty(&SaveFormat::V1(keyframes.to_vec()))?;
//         std::fs::write(file, json)?;
//     }
//     Ok(())
// }
//
// #[derive(Serialize, Deserialize)]
// pub enum SaveFormat {
//     V1(Vec<Keyframe>),
// }
//
// impl SaveFormat {
//     fn to_current(&self) -> Vec<Keyframe> {
//         match self {
//             SaveFormat::V1(keyframes) => keyframes.to_vec(),
//         }
//     }
// }
