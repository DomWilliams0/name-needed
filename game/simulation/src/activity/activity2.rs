use async_trait::async_trait;
use common::*;

#[async_trait]
pub trait Activity2: Display + Debug {
    // TODO need a context that can be stored forever
    async fn dew_it(&mut self) -> BoxedResult<()>;
}

// TODO temporary
#[derive(Default, Debug)]
pub struct TestActivity2;

// TODO temporary
#[derive(Default, Debug)]
pub struct NopActivity2;

#[async_trait]
impl Activity2 for TestActivity2 {
    async fn dew_it(&mut self) -> BoxedResult<()> {
        debug!("TODO wandering");
        Ok(())
    }
}

#[async_trait]
impl Activity2 for NopActivity2 {
    async fn dew_it(&mut self) -> BoxedResult<()> {
        // TODO reimplement nop
        Ok(())
    }
}

impl Display for NopActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Display for TestActivity2 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

// TODO ensure destructor runs when cancelled
