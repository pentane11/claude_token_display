// Windows でリリースビルド時にコンソールを開かないようにする
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    token_display_lib::run()
}
