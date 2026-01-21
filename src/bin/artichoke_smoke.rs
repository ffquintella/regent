use artichoke::prelude::Eval;

fn main() {
    eprintln!("Artichoke smoke: init");
    let mut interp = match artichoke::interpreter() {
        Ok(interp) => interp,
        Err(err) => {
            eprintln!("Artichoke smoke: init error: {err}");
            std::process::exit(1);
        }
    };
    eprintln!("Artichoke smoke: eval");
    if let Err(err) = interp.eval(b"1 + 1") {
        eprintln!("Artichoke smoke: eval error: {err}");
        std::process::exit(1);
    }
    eprintln!("Artichoke smoke: ok");
}
