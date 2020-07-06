use crate::society::job::JobList;
use common::*;

pub struct Society {
    name: String,
    jobs: JobList,
}

impl Society {
    pub(crate) fn with_name(name: String) -> Self {
        Self {
            name,
            jobs: JobList::default(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn jobs_mut(&mut self) -> &mut JobList {
        &mut self.jobs
    }
}

impl Debug for Society {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "Society({:?}, {} jobs)", self.name, self.jobs.count())
    }
}
