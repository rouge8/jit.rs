use jit::diff::diff;

fn main() {
    let a = "\
A
B
C
A
B
B
A";
    let b = "\
C
B
A
B
A
C";

    let edits = diff(a, b);
    for edit in edits {
        println!("{}", edit);
    }
}
