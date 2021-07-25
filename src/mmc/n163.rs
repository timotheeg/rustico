// Namco 163 (and also 129), reference capabilities:
// https://wiki.nesdev.com/w/index.php?title=INES_Mapper_019

use ines::INesCartridge;
use memoryblock::MemoryBlock;
use memoryblock::MemoryType;

use mmc::mapper::*;

use apu::AudioChannelState;
use apu::PlaybackRate;
use apu::Volume;
use apu::Timbre;
use apu::RingBuffer;

pub struct Namco163AudioChannel {
    pub debug_disable: bool,
    pub channel_address: usize,
    pub current_output: f64,
    // cache these to return for debugging purposes
    pub tracked_frequency: f64,
    pub tracked_volume: u8,
    pub output_buffer: RingBuffer,
}

const AUDIO_FREQ_LOW:     usize = 0;
const AUDIO_PHASE_LOW:    usize = 1;
const AUDIO_FREQ_MID:     usize = 2;
const AUDIO_PHASE_MID:    usize = 3;
const AUDIO_FREQ_HIGH:    usize = 4;
const AUDIO_WAVE_LENGTH:  usize = 4;
const AUDIO_PHASE_HIGH:   usize = 5;
const AUDIO_WAVE_ADDRESS: usize = 6;
const AUDIO_VOLUME:       usize = 7;

fn audio_sample(audio_ram: &[u8], sample_index: u8) -> u8 {
    let byte_index = sample_index / 2;
    let sample_byte = audio_ram[byte_index as usize];
    if sample_index & 0x1 == 0 {
        return sample_byte & 0x0F;
    } else {
        return (sample_byte & 0xF0) >> 4;
    }
}

impl Namco163AudioChannel {
    pub fn new(channel_address: usize) -> Namco163AudioChannel {
        return Namco163AudioChannel {
            debug_disable: false,
            channel_address: channel_address,
            current_output: 0.0,
            tracked_frequency: 0.0,
            tracked_volume: 0,
            output_buffer: RingBuffer::new(32768),
        }
    }

    pub fn phase(&self, audio_ram: &[u8]) -> u32 {
        let phase_low = audio_ram[self.channel_address + AUDIO_PHASE_LOW] as u32;
        let phase_mid = audio_ram[self.channel_address + AUDIO_PHASE_MID] as u32;
        let phase_high = audio_ram[self.channel_address + AUDIO_PHASE_HIGH] as u32;
        return (phase_high << 16) | (phase_mid << 8) | phase_low;
    }

    pub fn write_phase(&mut self, audio_ram: &mut [u8], phase: u32) {
        let phase_low = (phase & 0xFF) as u8;
        let phase_mid = ((phase & 0xFF00) >> 8) as u8;
        let phase_high = ((phase & 0xFF0000) >> 16) as u8;
        audio_ram[self.channel_address + AUDIO_PHASE_LOW] = phase_low;
        audio_ram[self.channel_address + AUDIO_PHASE_MID] = phase_mid;
        audio_ram[self.channel_address + AUDIO_PHASE_HIGH] = phase_high;
    }

    pub fn frequency(&self, audio_ram: &[u8]) -> u32 {
        let freq_low = audio_ram[self.channel_address + AUDIO_FREQ_LOW] as u32;
        let freq_mid = audio_ram[self.channel_address + AUDIO_FREQ_MID] as u32;
        let freq_high = (audio_ram[self.channel_address + AUDIO_FREQ_HIGH] & 0b0000_0011) as u32;
        return (freq_high << 16) | (freq_mid << 8) | freq_low;
    }

    pub fn length(&self, audio_ram: &[u8]) -> u32 {
        let length_byte = (audio_ram[self.channel_address + AUDIO_WAVE_LENGTH] & 0b1111_1100) as u32;
        return 256 - length_byte;
    }

    pub fn wave_address(&self, audio_ram: &[u8]) -> u8 {
        return audio_ram[self.channel_address + AUDIO_WAVE_ADDRESS];
    }

    pub fn volume(&self, audio_ram: &[u8]) -> u8 {
        return audio_ram[self.channel_address + AUDIO_VOLUME] & 0x0F;
    }

    pub fn update(&mut self, audio_ram: &mut [u8]) {
        let current_phase = self.phase(audio_ram);
        let frequency = self.frequency(audio_ram);
        let length = self.length(audio_ram);
        let sample_address = self.wave_address(audio_ram) as u32;
        let volume = self.volume(audio_ram);

        let new_phase = (current_phase + frequency) % (length << 16);
        self.write_phase(audio_ram, new_phase);

        let sample_index = (sample_address + (new_phase >> 16)) & 0xFF;
        let sample = audio_sample(audio_ram, sample_index as u8) as f64;

        // The final output sample is biased, such that +8 is centered
        self.current_output = (sample - 8.0) * (volume as f64);

        // for debug visualizations
        let ntsc_clockrate: f64 = 1_789_773.0;
        let enabled_channels = (((audio_ram[0x7F] & 0b0111_0000) >> 4) + 1) as u32;

        self.tracked_frequency = (ntsc_clockrate * (frequency as f64)) / (15.0 * 65536.0 * (length as f64) * (enabled_channels as f64));
        self.tracked_volume = volume;
    }
}

impl AudioChannelState for Namco163AudioChannel {
    fn name(&self) -> String {
        return format!("WAVE {}", ((self.channel_address - 0x40) / 8) + 1);
    }

    fn chip(&self) -> String {
        return "N163".to_string();
    }

    fn sample_buffer(&self) -> &RingBuffer {
        return &self.output_buffer;
    }

    fn record_current_output(&mut self) {
        self.output_buffer.push(self.current_output as i16);
    }

    fn min_sample(&self) -> i16 {
        return -128;
    }

    fn max_sample(&self) -> i16 {
        return 127;
    }

    fn muted(&self) -> bool {
        return self.debug_disable;
    }

    fn mute(&mut self) {
        self.debug_disable = true;
    }

    fn unmute(&mut self) {
        self.debug_disable = false;
    }

    fn playing(&self) -> bool {
        return 
            (self.tracked_volume > 0) &&
            (self.tracked_frequency > 0.0);
    }

    fn rate(&self) -> PlaybackRate {
        return PlaybackRate::FundamentalFrequency {frequency: self.tracked_frequency};
    }

    fn volume(&self) -> Option<Volume> {
        return Some(Volume::VolumeIndex{ index: self.tracked_volume as usize, max: 15 });
    }

    fn timbre(&self) -> Option<Timbre> {
        return None;
    }
}

pub struct Namco163Audio {
    pub internal_ram: Vec<u8>,
    pub channels: Vec<Namco163AudioChannel>,
    pub channel_delay_counter: u8,
    pub current_channel: usize,
    pub current_output: f64,
}

impl Namco163Audio {
    pub fn new() -> Namco163Audio {
        let mut chip = Namco163Audio {
            internal_ram: vec![0u8; 0x80],
            channels: Vec::new(),
            channel_delay_counter: 0,
            current_channel: 0,
            current_output: 0.0,
        };
        for i in 0 ..= 7 {
            chip.channels.push(Namco163AudioChannel::new(0x40 + (0x8 * (7 - i))));
        }
        return chip;
    }

    pub fn enabled_channels(&self) -> usize {
        let channel_cmp = (self.internal_ram[0x7F] & 0b0111_0000) >> 4;
        return (1 + channel_cmp) as usize;
    }

    pub fn clock(&mut self) {
        if self.channel_delay_counter > 0 {
            self.channel_delay_counter -= 1;
        }

        if self.channel_delay_counter == 0 {
            self.channels[self.current_channel].update(&mut self.internal_ram);
            if self.channels[self.current_channel].debug_disable {
                // Do debug muting here at the last second
                self.current_output = 0.0;
            } else {
                self.current_output = self.channels[self.current_channel].current_output;
            }
            self.current_channel += 1;
            if self.current_channel >= self.enabled_channels() {
                self.current_channel = 0;
            }

            self.channel_delay_counter = 15;
        }
    }

    pub fn record_output(&mut self) {
        for channel in &mut self.channels {
            channel.record_current_output();
        }
    }
}

pub struct Namco163 {
    pub prg_rom: MemoryBlock,
    pub prg_ram: MemoryBlock,
    pub chr: MemoryBlock,
    pub vram: MemoryBlock,
    pub expansion_audio_chip: Namco163Audio,

    pub irq_enabled: bool,
    pub irq_pending: bool,
    pub irq_counter: u16, // 15bit, actually

    pub chr_banks: Vec<u8>,
    pub nt_banks: Vec<u8>,
    pub prg_banks: Vec<u8>,

    pub internal_ram_addr: u8,
    pub internal_ram_auto_increment: bool,
    pub sound_enabled: bool,
    pub nt_ram_at_0000: bool,
    pub nt_ram_at_1000: bool,
}

impl Namco163 {
    pub fn from_ines(ines: INesCartridge) -> Result<Namco163, String> {
        let prg_rom_block = ines.prg_rom_block();
        let prg_ram_block = ines.prg_ram_block()?;
        let chr_block = ines.chr_block()?;

        return Ok(Namco163 {
            prg_rom: prg_rom_block.clone(),
            prg_ram: prg_ram_block.clone(),
            chr: chr_block.clone(),
            vram: MemoryBlock::new(&[0u8; 0x2000], MemoryType::Ram),
            expansion_audio_chip: Namco163Audio::new(),

            irq_enabled: false,
            irq_pending: false,
            irq_counter: 0,

            chr_banks: vec![0u8; 8],
            nt_banks: vec![0u8; 4],
            prg_banks: vec![0u8; 3],

            internal_ram_addr: 0, // upper nybble mismatch, will disable PRG RAM at boot
            internal_ram_auto_increment: false,
            sound_enabled: false,
            nt_ram_at_0000: false,
            nt_ram_at_1000: false,
        })
    }

    pub fn read_banked_chr(&self, address: u16, bank_index: u8, use_nt: bool) -> Option<u8> {
        if use_nt {
            let effective_bank_index = bank_index & 0x1;
            return self.vram.banked_read(0x400, effective_bank_index as usize, address as usize);
        } else {
            return self.chr.banked_read(0x400, bank_index as usize, address as usize);
        }
    }

    pub fn write_banked_chr(&mut self, address: u16, bank_index: u8, use_nt: bool, data: u8) {
        if use_nt {
            let effective_bank_index = bank_index & 0x1;
            self.vram.banked_write(0x400, effective_bank_index as usize, address as usize, data);
        } else {
            self.chr.banked_write(0x400, bank_index as usize, address as usize, data);
        }
    }

    pub fn prg_ram_write_enabled(&self, address: u16) -> bool {
        if self.internal_ram_addr & 0xF0 != 0b0100_0000 {
            return false;
        }
        let masked_address = address & 0xF800;
        match masked_address {
            0x6000 => (self.internal_ram_addr & 0b0000_0001) == 0,
            0x6800 => (self.internal_ram_addr & 0b0000_0010) == 0,
            0x7000 => (self.internal_ram_addr & 0b0000_0100) == 0,
            0x7800 => (self.internal_ram_addr & 0b0000_1000) == 0,
            _ => false
        }
    }




}

impl Mapper for Namco163 {
    fn mirroring(&self) -> Mirroring {
        return Mirroring::Horizontal;
    }
    
    fn debug_read_cpu(&self, address: u16) -> Option<u8> {
        match address {
            0x4800 ..= 0x4FFF => {
                Some(self.expansion_audio_chip.internal_ram[self.internal_ram_addr as usize])
            },
            0x5000 ..= 0x57FF => {
                let irq_low = (self.irq_counter & 0x00FF) as u8;
                Some(irq_low)
            },
            0x5800 ..= 0x5FFF => {
                let irq_high = ((self.irq_counter & 0xFF00) >> 8) as u8;
                let irq_enabled = if self.irq_enabled {0x80} else {0x00};
                Some(irq_high | irq_enabled)
            },
            0x6000 ..= 0x7FFF => self.prg_ram.wrapping_read(address as usize - 0x6000),
            0x8000 ..= 0x9FFF => self.prg_rom.banked_read(0x2000, self.prg_banks[0] as usize, address as usize),
            0xA000 ..= 0xBFFF => self.prg_rom.banked_read(0x2000, self.prg_banks[1] as usize, address as usize),
            0xC000 ..= 0xDFFF => self.prg_rom.banked_read(0x2000, self.prg_banks[2] as usize, address as usize),
            0xE000 ..= 0xFFFF => self.prg_rom.banked_read(0x2000, 0xFF, address as usize),
            _ => {None}
        }
    }

    fn read_cpu(&mut self, address: u16) -> Option<u8> {
        let data = self.debug_read_cpu(address);
        match address {
            0x4800 ..= 0x4FFF => {
                if self.internal_ram_auto_increment {
                    self.internal_ram_addr = (self.internal_ram_addr + 1) & 0x7F;
                }
            },
            _ => {}
        }
        return data;
    }

    fn debug_read_ppu(&self, address: u16) -> Option<u8> {
        let masked_address = address & 0xFC00;
        match masked_address {
            0x0000 => {self.read_banked_chr(address, self.chr_banks[0], self.nt_ram_at_0000)},
            0x0400 => {self.read_banked_chr(address, self.chr_banks[1], self.nt_ram_at_0000)},
            0x0800 => {self.read_banked_chr(address, self.chr_banks[2], self.nt_ram_at_0000)},
            0x0C00 => {self.read_banked_chr(address, self.chr_banks[3], self.nt_ram_at_0000)},
            0x1000 => {self.read_banked_chr(address, self.chr_banks[4], self.nt_ram_at_1000)},
            0x1400 => {self.read_banked_chr(address, self.chr_banks[5], self.nt_ram_at_1000)},
            0x1800 => {self.read_banked_chr(address, self.chr_banks[6], self.nt_ram_at_1000)},
            0x1C00 => {self.read_banked_chr(address, self.chr_banks[7], self.nt_ram_at_1000)},
            0x2000 => {self.read_banked_chr(address, self.nt_banks[0], true)},
            0x2400 => {self.read_banked_chr(address, self.nt_banks[1], true)},
            0x2800 => {self.read_banked_chr(address, self.nt_banks[2], true)},
            0x2C00 => {self.read_banked_chr(address, self.nt_banks[3], true)},
            _ => {None}
        }
    }

    fn write_ppu(&mut self, address: u16, data: u8) {
        let masked_address = address & 0xFC00;
        match masked_address {
            0x0000 => {self.write_banked_chr(address, self.chr_banks[0], self.nt_ram_at_0000, data)},
            0x0400 => {self.write_banked_chr(address, self.chr_banks[1], self.nt_ram_at_0000, data)},
            0x0800 => {self.write_banked_chr(address, self.chr_banks[2], self.nt_ram_at_0000, data)},
            0x0C00 => {self.write_banked_chr(address, self.chr_banks[3], self.nt_ram_at_0000, data)},
            0x1000 => {self.write_banked_chr(address, self.chr_banks[4], self.nt_ram_at_1000, data)},
            0x1400 => {self.write_banked_chr(address, self.chr_banks[5], self.nt_ram_at_1000, data)},
            0x1800 => {self.write_banked_chr(address, self.chr_banks[6], self.nt_ram_at_1000, data)},
            0x1C00 => {self.write_banked_chr(address, self.chr_banks[7], self.nt_ram_at_1000, data)},
            0x2000 => {self.write_banked_chr(address, self.nt_banks[0], true, data)},
            0x2400 => {self.write_banked_chr(address, self.nt_banks[1], true, data)},
            0x2800 => {self.write_banked_chr(address, self.nt_banks[2], true, data)},
            0x2C00 => {self.write_banked_chr(address, self.nt_banks[3], true, data)},
            _ => {}
        }
    }    

    fn write_cpu(&mut self, address: u16, data: u8) {
        let masked_address = address & 0xF800;
        match masked_address {
            0x4800 => {
                self.expansion_audio_chip.internal_ram[self.internal_ram_addr as usize] = data;
                if self.internal_ram_auto_increment {
                    self.internal_ram_addr = (self.internal_ram_addr + 1) & 0x7F;
                }
            },
            0x5000 => {
                let irq_low = data as u16;
                self.irq_counter = (self.irq_counter & 0xFF00) | irq_low;
                self.irq_pending = false;
            },
            0x5800 => {
                let irq_high = ((data as u16) & 0x7F) << 8;
                self.irq_counter = (self.irq_counter & 0x00FF) | irq_high;
                self.irq_enabled = (data & 0x80) != 0;
                self.irq_pending = false;
            },
            0x6000 ..= 0x7FFF => {
                if self.prg_ram_write_enabled(address) {
                    self.prg_ram.wrapping_write(address as usize - 0x6000, data);
                }
            },
            0x8000 => {self.chr_banks[0] = data;},
            0x8800 => {self.chr_banks[1] = data;},
            0x9000 => {self.chr_banks[2] = data;},
            0x9800 => {self.chr_banks[3] = data;},
            0xA000 => {self.chr_banks[4] = data;},
            0xA800 => {self.chr_banks[5] = data;},
            0xB000 => {self.chr_banks[6] = data;},
            0xB800 => {self.chr_banks[7] = data;},
            0xC000 => {self.nt_banks[0] = data;},
            0xC800 => {self.nt_banks[1] = data;},
            0xD000 => {self.nt_banks[2] = data;},
            0xD800 => {self.nt_banks[3] = data;},
            0xE000 => {
                self.prg_banks[0] = data & 0b0011_1111;
                self.sound_enabled = (data & 0b0100_0000) == 0;
            },
            0xE800 => {
                self.prg_banks[1] = data & 0b0011_1111;
                self.nt_ram_at_0000 = (data & 0b0100_0000) == 0;
                self.nt_ram_at_1000 = (data & 0b1000_0000) == 0;
            },
            0xF000 => {
                self.prg_banks[2] = data & 0b0011_1111;                
            }
            0xF800 => {
                self.internal_ram_addr = data & 0x7F;
                self.internal_ram_auto_increment = (data & 0b1000_0000) != 0;
            }
            _ => {}
        }
    }

    fn clock_cpu(&mut self) {
        if self.irq_enabled && self.irq_counter < 0x7FFF {
            self.irq_counter += 1;
            if self.irq_counter == 0x7FFF {
                self.irq_pending = true;
            }
        }
        self.expansion_audio_chip.clock();
    }

    fn mix_expansion_audio(&self, nes_sample: f64) -> f64 {
        let expansion_mix = self.expansion_audio_chip.current_output / 256.0; // for testing
        return nes_sample + expansion_mix;
    }

    fn record_expansion_audio_output(&mut self, _nes_sample: f64) {
        self.expansion_audio_chip.record_output();
    }

    fn channels(&self) ->  Vec<& dyn AudioChannelState> {
        let mut channels: Vec<& dyn AudioChannelState> = Vec::new();
        for i in 0 ..= 7 {
            channels.push(&self.expansion_audio_chip.channels[i]);
        }
        return channels;
    }
    /*
    fn channels_mut(&mut self) ->  Vec<&mut dyn AudioChannelState> {
        let mut channels: Vec<&mut dyn AudioChannelState> = Vec::new();
        for i in (0 ..= 7).rev() {
            channels.push(&mut (self.expansion_audio_chip.channels[i]));
        }
        return channels;
    }*/

    fn irq_flag(&self) -> bool {
        return self.irq_pending;
    }

    fn has_sram(&self) -> bool {
        return true;
    }

    fn get_sram(&self) -> Vec<u8> {
        return self.prg_ram.as_vec().clone();
    }

    fn load_sram(&mut self, sram_data: Vec<u8>) {
        *self.prg_ram.as_mut_vec() = sram_data;
    }
}
