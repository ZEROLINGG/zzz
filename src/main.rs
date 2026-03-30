
use zzz_core::shell::exec;

fn main() {
    println!("{:?}", exec("pwd","sh",None,None));
}
