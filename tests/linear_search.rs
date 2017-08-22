extern crate svm;

use svm::VirtualMachine;

// start:
//     c.li r4, %array                             # Set the pointer to the array
//     c.li r5, 10                                 # Set the length of the array
//     c.li r6, 4                                  # Set the number we're lookign for
//     c.li r2, %exit                              # Set the link register to exit
//     c.addi r0, $linear_search                   # Call linear_search function

// exit:
//     c.break                                     # Breakpoint so we can see index in r3
//     c.li r4, 0                                  # Set status to 0
//     c.call 0                                    # Call sys_exit(status)

// linear_search:
//     c.li r3, 0

// linear_search_loop:
//     load r7, r4, 0                              # Get item at pointer
//     beq r6, r7, $linear_search_exit             # If r7 == value jump to exit
//     c.addi r3, 1                                # Increment index
//     c.addi r4, 4                                # Increment pointer
//     beq r3, r5, $linear_search_exit_not_found   # If r3 == length jump to exit_not_found
//     c.addi r0, $linear_search_loop              # Jump to loop

// linear_search_exit_not_found:
//     c.li r3, -1

// linear_search_exit:
//     mv r0, r2                                   # Move link register to program counter

// array:                                          # NYI: Bytes must be appended manually to
// #    words 2, 14, 15, 1, 10, 4, 6, 18, 9, 8     # assembled binary

#[test]
fn linear_search() {
    let program = vec![
        0x31, 0x51, 0x71, 0x15, 0xb1, 0x09, 0xb1, 0x14, 0x13, 0x0c, 0x3f, 0x00, 0x31, 0x01, 0x3d, 0x00,
        0xf1, 0x00, 0xf4, 0x21, 0x00, 0x00, 0x24, 0x33, 0x07, 0x00, 0xd3, 0x02, 0x13, 0x09, 0xa4, 0x18,
        0x05, 0x00, 0x13, 0xdc, 0xf1, 0xfe, 0x39, 0x10, 0x02, 0x00, 0x00, 0x00, 0x0e, 0x00, 0x00, 0x00,
        0x0f, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
        0x06, 0x00, 0x00, 0x00, 0x12, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00
    ];

    let mut vm = VirtualMachine::new(program).unwrap();
    
    assert_eq!(vm.run(), Ok(0));
    assert_eq!(vm.registers[3], 5);
}
