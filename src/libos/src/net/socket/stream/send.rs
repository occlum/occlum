use super::*;

impl Ipv4StreamSocket {
    pub fn send(&self, buf: &[u8], flags: SendFlags) -> Result<usize> {
        self.sendto(buf, flags, &None)
    }

    pub fn sendmsg<'a, 'b>(&self, msg: &'b MsgHdr<'a>, flags: SendFlags) -> Result<usize> {
        todo!()
        // let msg_iov = msg.get_iovs();

        // self.do_sendmsg(
        //     msg_iov.as_slices(),
        //     flags,
        //     msg.get_name(),
        //     msg.get_control(),
        // )
    }

    pub(super) fn do_sendmsg(
        &self,
        data: &[&[u8]],
        flags: SendFlags,
        name: Option<&[u8]>,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        todo!()
        // let data_length = data.iter().map(|s| s.len()).sum();
        // let u_allocator = UntrustedSliceAlloc::new(data_length)?;
        // let u_data = {
        //     let mut bufs = Vec::new();
        //     for buf in data {
        //         bufs.push(u_allocator.new_slice(buf)?);
        //     }
        //     bufs
        // };

        // self.do_sendmsg_untrusted_data(&u_data, flags, name, control)
    }
}

