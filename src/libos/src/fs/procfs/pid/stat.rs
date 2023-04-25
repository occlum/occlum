use super::*;

pub struct ProcStatINode(ProcessRef);

impl ProcStatINode {
    pub fn new(process_ref: &ProcessRef) -> Arc<dyn AsyncInode> {
        Arc::new(File::new(Self(Arc::clone(process_ref))))
    }
}

#[async_trait]
impl ProcINode for ProcStatINode {
    async fn generate_data_in_bytes(&self) -> Result<Vec<u8>> {
        let main_thread = self.0.main_thread().ok_or(errno!(ENOENT, ""))?;

        // Get the process status information, some fields are filled with the
        // dummy value 0, while some fields are denies to access with value 0.
        // TODO: Fill in the dummy fields with meaningful values
        let pid = main_thread.tid();
        let comm = String::from_utf8(main_thread.name().as_c_str().to_bytes().to_vec()).unwrap();
        let state = match self.0.status() {
            ProcessStatus::Running => "R",
            ProcessStatus::Stopped => "T",
            ProcessStatus::Zombie => "Z",
        };
        let ppid = self.0.parent().pid();
        let pgrp = self.0.pgid();
        let session = pgrp.clone();
        let tty_nr = 0;
        let tpgid = pgrp.clone();
        let flags = 0;
        let minflt = 0;
        let cminflt = 0;
        let majflt = 0;
        let cmajflt = 0;
        let utime = 0;
        let stime = 0;
        let cutime = 0;
        let cstime = 0;
        let priority = main_thread.nice().read().unwrap().to_priority_val();
        let nice = main_thread.nice().read().unwrap().raw_val();
        let num_threads = self.0.threads().len();
        let itrealvalue = 0;
        let starttime = self.0.start_time();
        let vsize = main_thread.vm().get_process_range().size();
        let rss = 0;
        let rsslim = 0;
        let startcode = 0;
        let endcode = 0;
        let startstack = 0;
        let kstkesp = 0;
        let kstkeip = 0;
        let signal = 0;
        let blocked = 0;
        let sigignore = 0;
        let sigcatch = 0;
        let wchan = 0;
        let nswap = 0;
        let cnswap = 0;
        let exit_signal = 0;
        let processor = 0;
        let rt_priority = 0;
        let policy = 0;
        let delayacct_blkio_ticks = 0;
        let guest_time = 0;
        let cguest_time = 0;
        let start_data = 0;
        let end_data = 0;
        let start_brk = 0;
        let arg_start = 0;
        let arg_end = 0;
        let env_start = 0;
        let env_end = 0;
        let exit_code = 0;

        // Put the information together in the specific format
        let result = format!(
            "{} \
            ({}) \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {} \
            {}",
            pid,
            comm,
            state,
            ppid,
            pgrp,
            session,
            tty_nr,
            tpgid,
            flags,
            minflt,
            cminflt,
            majflt,
            cmajflt,
            utime,
            stime,
            cutime,
            cstime,
            priority,
            nice,
            num_threads,
            itrealvalue,
            starttime,
            vsize,
            rss,
            rsslim,
            startcode,
            endcode,
            startstack,
            kstkesp,
            kstkeip,
            signal,
            blocked,
            sigignore,
            sigcatch,
            wchan,
            nswap,
            cnswap,
            exit_signal,
            processor,
            rt_priority,
            policy,
            delayacct_blkio_ticks,
            guest_time,
            cguest_time,
            start_data,
            end_data,
            start_brk,
            arg_start,
            arg_end,
            env_start,
            env_end,
            exit_code
        )
        .into_bytes();
        Ok(result)
    }
}
