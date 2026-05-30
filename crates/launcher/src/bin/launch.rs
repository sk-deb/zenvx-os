//! `zenvx-launch [--dry] <spec>` — route and launch an app/file/disk image.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry = args.first().map(|a| a == "--dry").unwrap_or(false);
    let spec = if dry { args.get(1) } else { args.first() };
    let Some(spec) = spec else {
        eprintln!("usage: zenvx-launch [--dry] <app | file.exe | file.AppImage | app.id | disk.iso>");
        std::process::exit(2);
    };

    let res = if dry { zenvx_launcher::resolve(spec) } else { zenvx_launcher::launch(spec) };
    match res {
        Ok(cmd) => println!("{}{cmd}", if dry { "would run: " } else { "launching: " }),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
