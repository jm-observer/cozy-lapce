use log::info;
pub fn main() {
    let chars = ['\"', '(', ')', '[', ']', '{', '}', '!', ' ', '_', '\''];
    for c in chars {
        info!("{} {}", c, c.is_ascii_punctuation());
    }
    println!("fffffffffffff");
    println!("fffffffffffff");
    println!("fffffffffffff");
    println!("fffffffffffff");
}
