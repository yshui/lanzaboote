//! This module implements the protocols to hand an initrd to the
//! Linux kernel.

use core::{ffi::c_void, pin::Pin};

use alloc::boxed::Box;
use uefi::{
    prelude::BootServices,
    proto::{
        device_path::{DevicePath, FfiDevicePath},
        media::file::RegularFile,
        Protocol,
    },
    unsafe_guid, Handle, Identify, Result, Status,
};

/// The Linux kernel's initrd loading device path.
///
/// The Linux kernel points us to
/// [u-boot](https://github.com/u-boot/u-boot/commit/ec80b4735a593961fe701cc3a5d717d4739b0fd0#diff-1f940face4d1cf74f9d2324952759404d01ee0a81612b68afdcba6b49803bdbbR28)
/// for documentation.
// XXX This should actually be something like:
// static const struct {
// 	struct efi_vendor_dev_path	vendor;
// 	struct efi_generic_dev_path	end;
// } __packed initrd_dev_path = {
// 	{
// 		{
// 			EFI_DEV_MEDIA,
// 			EFI_DEV_MEDIA_VENDOR,
// 			sizeof(struct efi_vendor_dev_path),
// 		},
// 		LINUX_EFI_INITRD_MEDIA_GUID
// 	}, {
// 		EFI_DEV_END_PATH,
// 		EFI_DEV_END_ENTIRE,
// 		sizeof(struct efi_generic_dev_path)
// 	}
// };
static mut DEVICE_PATH_PROTOCOL: [u8; 24] = [
    0x04, 0x03, 0x14, 0x00, 0x27, 0xe4, 0x68, 0x55, 0xfc, 0x68, 0x3d, 0x4f, 0xac, 0x74, 0xca, 0x55,
    0x52, 0x31, 0xcc, 0x68, 0x7f, 0xff, 0x04, 0x00,
];

#[repr(C)]
#[unsafe_guid("4006c0c1-fcb3-403e-996d-4a6c8724e06d")]
#[derive(Protocol)]
struct LoadFile2Protocol {
    load_file: unsafe extern "efiapi" fn(
        this: &mut LoadFile2Protocol,
        file_path: *const FfiDevicePath,
        boot_policy: bool,
        buffer_size: *mut usize,
        buffer: *mut c_void,
    ) -> Status,

    // This is not part of the official protocol struct.
    file: RegularFile,
}

unsafe extern "efiapi" fn linux_initrd_load_file(
    _this: &mut LoadFile2Protocol,
    _file_path: *const FfiDevicePath,
    _boot_policy: bool,
    _buffer_size: *mut usize,
    _buffer: *mut c_void,
) -> Status {
    // Linux doesn't like that.
    Status::NOT_FOUND
}

pub struct InitrdLoader {
    proto: Pin<Box<LoadFile2Protocol>>,
    handle: Handle,
    registered: bool,
}

impl InitrdLoader {
    pub fn new(boot_services: &BootServices, handle: Handle, file: RegularFile) -> Result<Self> {
        let mut proto = Box::pin(LoadFile2Protocol {
            load_file: linux_initrd_load_file,
            file,
        });

        unsafe {
            let dp_proto: *mut u8 = &mut DEVICE_PATH_PROTOCOL[0];

            boot_services.install_protocol_interface(
                Some(handle),
                &DevicePath::GUID,
                dp_proto as *mut c_void,
            )?;

            let lf_proto: *mut LoadFile2Protocol = proto.as_mut().get_mut();

            boot_services.install_protocol_interface(
                Some(handle),
                &LoadFile2Protocol::GUID,
                lf_proto as *mut c_void,
            )?;
        }

        Ok(InitrdLoader {
            handle,
            proto,
            registered: true,
        })
    }

    pub fn uninstall(&mut self, boot_services: &BootServices) -> Result<()> {
        // This should only be called once.
        assert!(self.registered);

        unsafe {
            let dp_proto: *mut u8 = &mut DEVICE_PATH_PROTOCOL[0];
            boot_services.uninstall_protocol_interface(
                self.handle,
                &DevicePath::GUID,
                dp_proto as *mut c_void,
            )?;

            let lf_proto: *mut LoadFile2Protocol = self.proto.as_mut().get_mut();

            boot_services.uninstall_protocol_interface(
                self.handle,
                &LoadFile2Protocol::GUID,
                lf_proto as *mut c_void,
            )?;
        }

        self.registered = false;

        Ok(())
    }
}

impl Drop for InitrdLoader {
    fn drop(&mut self) {
        // Dropped without unregistering!
        assert!(!self.registered);
    }
}
