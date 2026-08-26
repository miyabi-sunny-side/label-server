//! PT-P700 raster protocol.
//!
//! Ported from libptouch.c of ptouch-print,
//! Copyright (C) 2013-2026 Dominic Radermacher <dominic@familie-radermacher.ch>,
//! licensed under the GNU General Public License version 3.
//! This port is part of label-server and is likewise GPL-3.0-only.

use std::{fmt, thread, time::Duration};

use crate::render::Bitmap;

/// Width of the PT-P700 print head in pixels.
pub const MAX_PX: usize = 128;
const BYTES_PER_LINE: usize = MAX_PX / 8;
#[allow(clippy::cast_possible_truncation)]
const BYTES_PER_LINE_U8: u8 = BYTES_PER_LINE as u8;

pub const VENDOR_ID: u16 = 0x04f9;
pub const PRODUCT_ID: u16 = 0x2061;
/// The same printer while its Editor Lite lamp is on (a USB disk, cannot print).
pub const PRODUCT_ID_EDITOR_LITE: u16 = 0x2064;

const ENDPOINT_OUT: u8 = 0x02;
const ENDPOINT_IN: u8 = 0x81;
/// Status polls before giving up (each read waits up to `READ_TIMEOUT`).
const STATUS_POLLS: usize = 50;
const READ_TIMEOUT: Duration = Duration::from_secs(1);

/// `ESC i S` — status information request.
pub const STATUS_REQUEST: [u8; 3] = [0x1b, 0x69, 0x53];
/// `M 02` — enable `PackBits` compression (the raster packets fake it with
/// one uncompressed run).
pub const PACKBITS_ENABLE: [u8; 2] = [0x4d, 0x02];
/// `ESC i a 01` — switch to raster mode (P700 family).
pub const RASTER_MODE: [u8; 4] = [0x1b, 0x69, 0x61, 0x01];
/// `ESC i M 40` — cut the blank leader before printing.
pub const PRECUT: [u8; 4] = [0x1b, 0x69, 0x4d, 0x40];
/// Print with feeding (eject / cut).
pub const EJECT: [u8; 1] = [0x1a];

/// 100 invalidation bytes followed by `ESC @` (initialise).
#[must_use]
pub fn init_command() -> Vec<u8> {
    let mut cmd = vec![0u8; 100];
    cmd.extend_from_slice(&[0x1b, 0x40]);
    cmd
}

#[derive(Debug)]
pub enum Error {
    Usb(rusb::Error),
    NotFound,
    EditorLite,
    BadStatus(String),
    Render(String),
    TooTall { height: usize, tape_px: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usb(err) => write!(f, "USB error: {err}"),
            Self::NotFound => write!(f, "no PT-P700 found on USB"),
            Self::EditorLite => write!(
                f,
                "PT-P700 is in Editor Lite mode; hold the Editor Lite button until its lamp goes out"
            ),
            Self::BadStatus(detail) => write!(f, "unexpected printer status: {detail}"),
            Self::Render(detail) => f.write_str(detail),
            Self::TooTall { height, tape_px } => write!(
                f,
                "label is {height}px tall but the tape prints at most {tape_px}px"
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusb::Error> for Error {
    fn from(err: rusb::Error) -> Self {
        Self::Usb(err)
    }
}

/// Bulk transfers to and from the printer. The USB implementation is
/// [`UsbTransport`]; tests substitute a recorder.
pub trait Transport {
    /// Sends one bulk packet.
    ///
    /// # Errors
    /// Returns [`Error::Usb`] when the transfer fails or is short.
    fn write(&mut self, data: &[u8]) -> Result<(), Error>;
    /// Reads one bulk packet; `Ok(0)` means nothing arrived yet.
    ///
    /// # Errors
    /// Returns [`Error::Usb`] when the transfer fails.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub media_width_mm: u8,
    pub tape_px: usize,
}

/// Printable pixels for a tape width in mm (the tapes a PT-P700 accepts).
#[must_use]
pub fn tape_width_px(mm: u8) -> Option<usize> {
    match mm {
        4 => Some(24),
        6 => Some(32),
        9 => Some(52),
        12 => Some(76),
        18 => Some(120),
        21 => Some(124),
        24 => Some(128),
        _ => None,
    }
}

/// Decodes the 32-byte status frame (`80 20 ...`, media width at byte 10).
///
/// # Errors
/// Returns [`Error::BadStatus`] for a wrong length, header, or tape width.
pub fn parse_status(frame: &[u8]) -> Result<Status, Error> {
    if frame.len() != 32 {
        return Err(Error::BadStatus(format!(
            "got {} bytes instead of 32",
            frame.len()
        )));
    }
    if frame[0] != 0x80 || frame[1] != 0x20 {
        return Err(Error::BadStatus(format!(
            "header {:02x} {:02x}",
            frame[0], frame[1]
        )));
    }
    let media_width_mm = frame[10];
    let tape_px = tape_width_px(media_width_mm)
        .filter(|&px| px <= MAX_PX)
        .ok_or_else(|| Error::BadStatus(format!("unsupported tape width {media_width_mm}mm")))?;
    Ok(Status {
        media_width_mm,
        tape_px,
    })
}

/// Asks the printer which tape is loaded, polling the transport up to
/// `STATUS_POLLS` times.
///
/// # Errors
/// Returns the transport error, or [`Error::BadStatus`] when the printer
/// answers with something else or not at all.
pub fn query_status<T: Transport>(transport: &mut T) -> Result<Status, Error> {
    transport.write(&STATUS_REQUEST)?;
    let mut buf = [0u8; 32];
    for _ in 0..STATUS_POLLS {
        thread::sleep(Duration::from_millis(100));
        let n = transport.read(&mut buf)?;
        if n > 0 {
            return parse_status(&buf[..n]);
        }
    }
    Err(Error::BadStatus(format!(
        "no response after {STATUS_POLLS} polls"
    )))
}

/// One raster packet for a bitmap column, centred on the print head.
fn raster_packet(bitmap: &Bitmap, column: usize, offset: usize) -> Vec<u8> {
    let mut packet = vec![0x47, BYTES_PER_LINE_U8 + 1, 0x00, BYTES_PER_LINE_U8 - 1];
    let mut line = [0u8; BYTES_PER_LINE];
    for row in 0..bitmap.height {
        if bitmap.get(column, bitmap.height - 1 - row) {
            let bit = offset + row;
            line[bit / 8] |= 0x80 >> (bit % 8);
        }
    }
    packet.extend_from_slice(&line);
    packet
}

/// Initialises the printer, reads the tape, asks `make_bitmap` for the
/// label (rows = tape width, columns = tape length), prints it and ejects.
/// This is the whole production send sequence; [`UsbPrinter`] adds nothing.
///
/// [`UsbPrinter`]: crate::UsbPrinter
///
/// # Errors
/// Returns the transport or status error, whatever `make_bitmap` returned
/// (as [`Error::Render`]), or [`Error::TooTall`] when the bitmap does not
/// fit the loaded tape.
pub fn print<T, F>(transport: &mut T, precut: bool, make_bitmap: F) -> Result<Status, Error>
where
    T: Transport,
    F: FnOnce(&Status) -> Result<Bitmap, String>,
{
    transport.write(&init_command())?;
    let status = query_status(transport)?;
    let bitmap = make_bitmap(&status).map_err(Error::Render)?;
    if bitmap.height > status.tape_px {
        return Err(Error::TooTall {
            height: bitmap.height,
            tape_px: status.tape_px,
        });
    }
    transport.write(&PACKBITS_ENABLE)?;
    transport.write(&RASTER_MODE)?;
    if precut {
        transport.write(&PRECUT)?;
    }
    let offset = (MAX_PX - bitmap.height) / 2;
    for column in 0..bitmap.width {
        transport.write(&raster_packet(&bitmap, column, offset))?;
    }
    transport.write(&EJECT)?;
    Ok(status)
}

/// libusb-backed transport to the first PT-P700 on the bus.
pub struct UsbTransport {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
}

impl UsbTransport {
    /// Claims interface 0 of the first PT-P700 on the bus.
    ///
    /// # Errors
    /// Returns [`Error::NotFound`] when no printer is attached,
    /// [`Error::EditorLite`] when it is attached as a USB disk, or the
    /// libusb error from opening or claiming it.
    pub fn open() -> Result<Self, Error> {
        let mut editor_lite = false;
        for device in rusb::devices()?.iter() {
            let desc = device.device_descriptor()?;
            if desc.vendor_id() != VENDOR_ID {
                continue;
            }
            match desc.product_id() {
                PRODUCT_ID => {
                    let handle = device.open()?;
                    if handle.kernel_driver_active(0).unwrap_or(false) {
                        handle.detach_kernel_driver(0)?;
                    }
                    handle.claim_interface(0)?;
                    return Ok(Self { handle });
                }
                PRODUCT_ID_EDITOR_LITE => editor_lite = true,
                _ => {}
            }
        }
        Err(if editor_lite {
            Error::EditorLite
        } else {
            Error::NotFound
        })
    }
}

impl Transport for UsbTransport {
    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        let sent = self
            .handle
            .write_bulk(ENDPOINT_OUT, data, Duration::from_secs(10))?;
        if sent == data.len() {
            Ok(())
        } else {
            Err(Error::Usb(rusb::Error::Io))
        }
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        match self.handle.read_bulk(ENDPOINT_IN, buf, READ_TIMEOUT) {
            Ok(n) => Ok(n),
            Err(rusb::Error::Timeout) => Ok(0),
            Err(err) => Err(Error::Usb(err)),
        }
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(0);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{Error, PRECUT, Transport, parse_status, print, tape_width_px};
    use crate::render::Bitmap;

    /// Records every bulk write and answers status reads with a canned frame.
    struct FakeTransport {
        writes: RefCell<Vec<Vec<u8>>>,
        status: Vec<u8>,
    }

    impl FakeTransport {
        fn with_media(mm: u8) -> Self {
            let mut status = vec![0u8; 32];
            status[0] = 0x80;
            status[1] = 0x20;
            status[10] = mm;
            Self {
                writes: RefCell::new(Vec::new()),
                status,
            }
        }
    }

    impl Transport for &FakeTransport {
        fn write(&mut self, data: &[u8]) -> Result<(), Error> {
            self.writes.borrow_mut().push(data.to_vec());
            Ok(())
        }

        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
            buf[..self.status.len()].copy_from_slice(&self.status);
            Ok(self.status.len())
        }
    }

    fn status_frame(mm: u8) -> [u8; 32] {
        let mut frame = [0u8; 32];
        frame[0] = 0x80;
        frame[1] = 0x20;
        frame[10] = mm;
        frame
    }

    #[test]
    fn status_reports_the_printable_width_of_the_loaded_tape() {
        assert_eq!(parse_status(&status_frame(12)).unwrap().tape_px, 76);
        assert_eq!(parse_status(&status_frame(24)).unwrap().tape_px, 128);
        assert_eq!(tape_width_px(9), Some(52));
        assert_eq!(tape_width_px(99), None);
    }

    #[test]
    fn status_with_a_wrong_header_is_rejected() {
        let mut frame = status_frame(12);
        frame[0] = 0x00;
        assert!(matches!(parse_status(&frame), Err(Error::BadStatus(_))));
        assert!(matches!(
            parse_status(&[0x80, 0x20]),
            Err(Error::BadStatus(_))
        ));
    }

    #[test]
    fn tapes_wider_than_the_print_head_are_rejected() {
        // 36mm tape would be 192px on wider printers; a PT-P700 cannot load it
        assert!(matches!(
            parse_status(&status_frame(36)),
            Err(Error::BadStatus(_))
        ));
    }

    #[test]
    fn printing_sends_the_ptouch_print_command_sequence() {
        let transport = FakeTransport::with_media(12);
        // 2 columns x 4 rows: column 0 inks the bottom row, column 1 the top.
        let mut bitmap = Bitmap::new(2, 4);
        bitmap.set(0, 3);
        bitmap.set(1, 0);

        let mut seen_status = None;
        let status = print(&mut &transport, true, |status| {
            seen_status = Some(*status);
            Ok(bitmap.clone())
        })
        .unwrap();
        assert_eq!(status.media_width_mm, 12);
        assert_eq!(seen_status.unwrap().tape_px, 76);

        let writes = transport.writes.borrow();
        let mut init = vec![0u8; 100];
        init.extend_from_slice(&[0x1b, 0x40]);
        assert_eq!(writes[0], init);
        assert_eq!(writes[1], [0x1b, 0x69, 0x53]);
        assert_eq!(writes[2], [0x4d, 0x02]);
        assert_eq!(writes[3], [0x1b, 0x69, 0x61, 0x01]);
        assert_eq!(writes[4], [0x1b, 0x69, 0x4d, 0x40]);
        // Golden raster packets from the ptouch-print protocol: 'G', 17, 0, 15
        // then 16 bytes. A 4px image sits at bits 62..=65 of the 128px head;
        // rows go out bottom-up, so the bottom row is bit 62 (byte 7, 0x02)
        // and the top row is bit 65 (byte 8, 0x40).
        assert_eq!(
            writes[5],
            [
                0x47, 0x11, 0x00, 0x0f, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ]
        );
        assert_eq!(
            writes[6],
            [
                0x47, 0x11, 0x00, 0x0f, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ]
        );
        assert_eq!(writes[7], [0x1a]);
        assert_eq!(writes.len(), 8);
        assert!(writes.iter().all(|packet| packet.len() <= 128));
    }

    #[test]
    fn printing_refuses_a_bitmap_taller_than_the_tape() {
        let transport = FakeTransport::with_media(12);

        let err = print(&mut &transport, true, |_| Ok(Bitmap::new(1, 77))).unwrap_err();
        assert!(matches!(
            err,
            Error::TooTall {
                height: 77,
                tape_px: 76
            }
        ));
        // nothing beyond init and the status request went to the printer
        assert_eq!(transport.writes.borrow().len(), 2);
    }

    #[test]
    fn a_render_failure_stops_before_any_raster_data() {
        let transport = FakeTransport::with_media(12);

        let err = print(&mut &transport, true, |_| Err("no glyphs".to_owned())).unwrap_err();
        assert!(matches!(err, Error::Render(ref m) if m == "no glyphs"));
        assert_eq!(transport.writes.borrow().len(), 2);
    }

    #[test]
    fn printing_without_precut_skips_the_precut_command() {
        let transport = FakeTransport::with_media(24);
        print(&mut &transport, false, |_| Ok(Bitmap::new(1, 1))).unwrap();
        assert!(!transport.writes.borrow().contains(&PRECUT.to_vec()));
    }
}
