use super::*;
use crate::untrusted::{SliceAsMutPtrAndLen, SliceAsPtrAndLen, UntrustedSliceAlloc};

impl Ipv4StreamSocket {
    pub fn recv(&self, buf: &mut [u8], flags: RecvFlags) -> Result<usize> {
        todo!()
        // let (bytes_recvd, _) = self.recvfrom(buf, flags)?;
        // Ok(bytes_recvd)
    }

    pub fn recvmsg<'a, 'b>(&self, msg: &'b mut MsgHdrMut<'a>, flags: RecvFlags) -> Result<usize> {
        todo!()
        // // Do OCall-based recvmsg
        // let (bytes_recvd, namelen_recvd, controllen_recvd, flags_recvd) = {
        //     // Acquire mutable references to the name and control buffers
        //     let (iovs, name, control) = msg.get_iovs_name_and_control_mut();
        //     // Fill the data, the name, and the control buffers
        //     self.do_recvmsg(iovs.as_slices_mut(), flags, name, control)?
        // };

        // // Update the output lengths and flags
        // msg.set_name_len(namelen_recvd)?;
        // msg.set_control_len(controllen_recvd)?;
        // msg.set_flags(flags_recvd);

        // Ok(bytes_recvd)
    }

    pub(super) fn do_recvmsg(
        &self,
        data: &mut [&mut [u8]],
        flags: RecvFlags,
        mut name: Option<&mut [u8]>,
        mut control: Option<&mut [u8]>,
    ) -> Result<(usize, usize, usize, MsgHdrFlags)> {
        todo!()
        // let data_length = data.iter().map(|s| s.len()).sum();
        // let u_allocator = UntrustedSliceAlloc::new(data_length)?;
        // let mut u_data = {
        //     let mut bufs = Vec::new();
        //     for ref buf in data.iter() {
        //         bufs.push(u_allocator.new_slice_mut(buf.len())?);
        //     }
        //     bufs
        // };
        // let retval = self.do_recvmsg_untrusted_data(&mut u_data, flags, name, control)?;

        // let mut remain = retval.0;
        // for (i, buf) in data.iter_mut().enumerate() {
        //     if remain >= buf.len() {
        //         u_data[i].write_to_slice(buf)?;
        //         remain -= buf.len();
        //     } else {
        //         u_data[i].write_to_slice(&mut buf[0..remain])?;
        //         break;
        //     }
        // }
        // Ok(retval)
    }
}

