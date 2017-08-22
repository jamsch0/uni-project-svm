extern crate svm;

use svm::VirtualMachine;

// start:
//     c.li r4, 47                     # Set the index of the fibonacci number we're looking for
//     c.li r2, %exit                  # Set the link register to exit
//     c.addi r0, $fibonacci           # Call fibonacci function
//
// exit:
//     c.break                         # Breakpoint so we can see fibonacci number in r3
//     c.li r4, 0                      # Set status to 0
//     c.call 0                        # Call sys_exit(status)
//
// fibonacci:
//     c.li r3, 0
//     c.li r5, 0
//     c.li r6, 1
//
// fibonacci_loop:
//     c.addi r4, -2                   # Sub 2 from r4
//     blt r4, r3, $fibonacci_exit     # If r4 < 0 jump to exit
//     c.add r5, r6                    # Add r6 to r5
//     c.add r6, r5                    # Add r5 to r6
//     c.addi r0, $fibonacci_loop      # Jump to loop
//
// fibonacci_exit_even:
//     mv r3, r5                       # Move r5 to r3 (return register)
//     mv r0, r2                       # Move link register to program counter
//
// fibonacci_exit:
//     c.andi r4, 1
//     c.bez r4, $fibonacci_exit_even  # If r4 & 1 == 0 jump to exit_even
//     mv r3, r6                       # Move r6 to return register
//     mv r0, r2                       # Move link register to program counter

#[test]
fn fibonacci() {
    let program = vec![
        0x31, 0x5f, 0xb1, 0x0c, 0x13, 0x0c, 0x3f, 0x00, 0x31, 0x01, 0x3d, 0x00, 0xf1, 0x00, 0x71, 0x01,
        0xb1, 0x03, 0x13, 0xfd, 0xa8, 0x22, 0x03, 0x00, 0x43, 0x31, 0x83, 0x29, 0x13, 0xe8, 0xf9, 0x28,
        0x39, 0x10, 0x15, 0x03, 0x21, 0xf1, 0xf9, 0x30, 0x39, 0x10
    ];

    let mut vm = VirtualMachine::new(program).unwrap();

    assert_eq!(vm.run(), Ok(0));
    assert_eq!(vm.registers[3], 2971215073);
}
