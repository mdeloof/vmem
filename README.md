## VMem

`VMem` provides a virtual memory data strucure where physical memory is allocated as it is written to.

## Example

```rust
use vmem::VMem;

// Create a new virtual memory that spans the addresses 0x00 to 0xff,
// where each word is 4 bytes wide.
let mut vmem = VMem::<4>::new(0x100);

// Write a word to the specified address.
vmem.write_word([0x01, 0x02, 0x04, 0x08], 0x03).unwrap();

// Read word from the specified address.
let word = vmem.read_word(0x03);
```