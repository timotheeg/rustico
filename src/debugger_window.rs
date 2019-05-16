extern crate sdl2;

use rusticnes_core::nes::NesState;
use rusticnes_core::opcode_info::disassemble_instruction;
use rusticnes_core::memory;

use drawing;
use drawing::Font;
use drawing::SimpleBuffer;

pub struct DebuggerWindow {
  pub buffer: SimpleBuffer,
  pub shown: bool,
  pub font: Font,
}

impl DebuggerWindow {
  pub fn new() -> DebuggerWindow {
    let font = Font::from_raw(include_bytes!("assets/8x8_font.png"), 8);

    return DebuggerWindow {
      buffer: SimpleBuffer::new(256, 300),
      font: font,
      shown: false,
    }
  }

  pub fn draw_registers(&mut self, nes: &mut NesState, x: u32, y: u32) {
    drawing::text(&mut self.buffer, &self.font, x, y, 
      "===== Registers =====", 
      &[192, 192, 192, 255]);
    drawing::text(&mut self.buffer, &self.font, x, y + 8, 
      &format!("A: 0x{:02X}", nes.registers.a), &[255, 255, 128, 255]);
    drawing::text(&mut self.buffer, &self.font, x, y + 16, 
      &format!("X: 0x{:02X}", nes.registers.x), &[160, 160, 160, 255]);
    drawing::text(&mut self.buffer, &self.font, x, y + 24, 
      &format!("Y: 0x{:02X}", nes.registers.y), &[224, 224, 224, 255]);

    drawing::text(&mut self.buffer, &self.font, x + 64, y + 8, 
      &format!("PC: 0x{:04X}", nes.registers.pc), &[255, 128, 128, 255]);
    drawing::text(&mut self.buffer, &self.font, x + 64, y + 16, 
      &format!("S:      {:02X}", nes.registers.s), &[128, 128, 255, 255]);
    drawing::text(&mut self.buffer, &self.font, x + 64, y + 16, 
               "    0x10  ",                       &[128, 128, 255, 160]);
    drawing::text(&mut self.buffer, &self.font, x + 64, y + 24, 
               "F:  nvdzic", &[128, 192, 128, 64]);
    drawing::text(&mut self.buffer, &self.font, x + 64, y + 24, 
      &format!("F:  {}{}{}{}{}{}",
        if nes.registers.flags.negative            {"n"} else {" "},
        if nes.registers.flags.overflow            {"v"} else {" "},
        if nes.registers.flags.decimal             {"d"} else {" "},
        if nes.registers.flags.zero                {"z"} else {" "},
        if nes.registers.flags.interrupts_disabled {"i"} else {" "},
        if nes.registers.flags.carry               {"c"} else {" "}),
      &[128, 192, 128, 255]);
  }

  pub fn draw_disassembly(&mut self, nes: &mut NesState, x: u32, y: u32) {
    drawing::text(&mut self.buffer, &self.font, x, y, 
    "===== Disassembly =====", &[255, 255, 255, 255]);

    let mut data_bytes_to_skip = 0;
    for i in 0 .. 30 {
      let pc = nes.registers.pc + (i as u16);
      let opcode = memory::passively_read_byte(nes, pc);
      let data1 = memory::passively_read_byte(nes, pc + 1);
      let data2 = memory::passively_read_byte(nes, pc + 2);
      let (instruction, data_bytes) = disassemble_instruction(opcode, data1, data2);
      let mut text_color = [255, 255, 255, 255];
      if data_bytes_to_skip > 0 {
        text_color = [64, 64, 64, 255];
        data_bytes_to_skip -= 1;
      } else {
        data_bytes_to_skip = data_bytes;
      }

      drawing::text(&mut self.buffer, &self.font, x, y + 16 + (i as u32 * 8),
        &format!("0x{:04X} - 0x{:02X}:  {}", pc, opcode, instruction),
        &text_color);     
    }
  }

  pub fn update(&mut self, nes: &mut NesState) {
    // Clear!
    let width = self.buffer.width;
    let height = self.buffer.height;
    drawing::rect(&mut self.buffer, 0, 0, width, height, &[0,0,0,255]);
    self.draw_registers(nes, 0, 0);
    self.draw_disassembly(nes, 0, 40);
  }
}

