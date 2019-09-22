use gameloop::GameLoop;
use std::time::Duration;

#[test]
#[ignore]
fn run_loop() {
    let gameloop = GameLoop::new(20, 5);

    for f in 0..10 {
        println!("--- start frame {}", f);
        std::thread::sleep(Duration::from_millis(153));

        let frame = gameloop.start_frame();
        for (i, action) in frame.actions().enumerate() {
            println!("{}): {:?}", i, action);
        }
    }
}
