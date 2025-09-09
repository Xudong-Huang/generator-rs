use windows_bindgen::bindgen;

fn main() {
    bindgen(["--etc", "bindings.txt"]).unwrap()
}
