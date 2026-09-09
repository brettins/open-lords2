// RecordProbe - what does the binary actually touch inside a record array, and how wide?
//
// Why this exists
// ---------------
// Before a record array can be given a struct type, the field map has to be
// right. docs/kingdom.md, docs/armies.md and the Rust structs say *where* the
// fields are; they mostly do not say *how wide* they are, and a field typed one
// byte too wide swallows its neighbour and propagates that mistake into every
// function that touches it (correction C3).
//
// The binary knows the width: an instruction that reads county+0x0C reads it as
// a byte, a word or a dword and the p-code varnode carries that size. So this
// script walks every reference that lands inside a record array, folds the
// target address to an offset within the record, and reports the sizes seen at
// each offset with a count of how many instructions used each.
//
// It names nothing and changes nothing. It is the check the struct definitions
// are written against, and the place a contradiction shows up.
//
// Usage:
//   analyzeHeadless <proj> <name> -process Lords2.exe -noanalysis \
//       -scriptPath ghidra_scripts -postScript RecordProbe <out.tsv>
//
// Output, one line per (record, offset):
//   record <TAB> offset <TAB> refs <TAB> size:count,size:count...
//
//@category Lords2

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.pcode.PcodeOp;
import ghidra.program.model.pcode.Varnode;

import java.io.File;
import java.io.PrintWriter;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;

public class RecordProbe extends GhidraScript {

    /** name, base, stride, count - the five arrays this project has layouts for. */
    private static final Object[][] ARRAYS = {
        { "county", 0x0053F9B0L, 0x300L, 17L },
        { "unit",   0x0052F0B0L, 0x1A4L, 151L },
        { "realm",  0x0057BF00L, 0x160L, 6L },
        { "lord",   0x004D8A58L, 0xF0L, 4L },
        { "battleMan",  0x00554480L, 0x1B0L, 80L },
        { "battleUnit", 0x00566520L, 0x34L, 80L },
        // The two grids are byte-offset indexed rather than stride indexed, but
        // the fold this script does - address minus base, modulo stride - is the
        // same arithmetic either way, so an 8-byte "stride" reports the plane.
        { "tile",       0x00522F90L, 0x8L,  4096L },
        { "battleCell", 0x005440E0L, 0x8L,  6400L },
    };

    /** record -> offset -> size -> count */
    private final Map<String, TreeMap<Long, TreeMap<Integer, Integer>>> hits = new LinkedHashMap<>();
    /** record -> offset -> distinct record indices touched */
    private final Map<String, TreeMap<Long, java.util.TreeSet<Long>>> slots = new LinkedHashMap<>();

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File out = args.length > 0 ? new File(args[0]) : null;

        for (Object[] a : ARRAYS) {
            hits.put((String) a[0], new TreeMap<>());
            slots.put((String) a[0], new TreeMap<>());
        }

        AddressSet exec = new AddressSet();
        for (ghidra.program.model.mem.MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            if (b.isExecute()) exec.addRange(b.getStart(), b.getEnd());
        }

        int scanned = 0;
        InstructionIterator it = currentProgram.getListing().getInstructions(exec, true);
        while (it.hasNext()) {
            if (monitor.isCancelled()) break;
            Instruction ins = it.next();
            scanned++;
            // A record array is *indexed*: `mov al,[eax + 0x53f9b5]`. The base
            // address is then only a displacement in the operand, and the
            // access width lives on the p-code LOAD/STORE the instruction
            // expands to. So: collect the in-range addresses this instruction
            // mentions, work out the one width it accesses, and pair them.
            java.util.List<Long> targets = new java.util.ArrayList<>();
            for (int i = 0; i < ins.getNumOperands(); i++) {
                for (Object o : ins.getOpObjects(i)) {
                    if (o instanceof ghidra.program.model.scalar.Scalar) {
                        targets.add(((ghidra.program.model.scalar.Scalar) o).getUnsignedValue());
                    } else if (o instanceof Address) {
                        targets.add(((Address) o).getOffset());
                    }
                }
            }
            if (targets.isEmpty()) continue;

            // 0 means "mentioned but never dereferenced here" - lea, push, a
            // pointer handed to a callee. Kept distinct from a real access.
            java.util.TreeSet<Integer> widths = new java.util.TreeSet<>();
            for (PcodeOp op : ins.getPcode()) {
                if (op.getOpcode() == PcodeOp.LOAD && op.getOutput() != null) {
                    widths.add(op.getOutput().getSize());
                } else if (op.getOpcode() == PcodeOp.STORE && op.getNumInputs() > 2) {
                    widths.add(op.getInput(2).getSize());
                } else {
                    for (Varnode v : inputsAndOutput(op)) {
                        if (v != null && v.isAddress()) widths.add(v.getSize());
                    }
                }
            }
            int width = widths.size() == 1 ? widths.first() : (widths.isEmpty() ? 0 : -1);
            for (Long t : targets) record(t, width);
        }

        PrintWriter w = out == null ? null : new PrintWriter(out, "UTF-8");
        println("RecordProbe: scanned " + scanned + " instructions");
        for (Object[] a : ARRAYS) {
            String name = (String) a[0];
            TreeMap<Long, TreeMap<Integer, Integer>> m = hits.get(name);
            println("  " + name + ": " + m.size() + " distinct offsets touched");
            if (w == null) continue;
            for (Map.Entry<Long, TreeMap<Integer, Integer>> e : m.entrySet()) {
                StringBuilder sb = new StringBuilder();
                int refs = 0;
                for (Map.Entry<Integer, Integer> s : e.getValue().entrySet()) {
                    if (sb.length() > 0) sb.append(",");
                    sb.append(s.getKey()).append(":").append(s.getValue());
                    refs += s.getValue();
                }
                w.printf("%s\t0x%03X\t%d\t%d\t%s%n", name, e.getKey(), refs,
                        slots.get(name).get(e.getKey()).size(), sb);
            }
        }
        if (w != null) {
            w.close();
            println("wrote " + out.getAbsolutePath());
        }
    }

    private Varnode[] inputsAndOutput(PcodeOp op) {
        Varnode[] in = op.getInputs();
        Varnode[] all = new Varnode[in.length + 1];
        System.arraycopy(in, 0, all, 0, in.length);
        all[in.length] = op.getOutput();
        return all;
    }

    private void record(long addr, int size) {
        for (Object[] a : ARRAYS) {
            String name = (String) a[0];
            long base = (Long) a[1], stride = (Long) a[2], count = (Long) a[3];
            if (addr < base || addr >= base + stride * count) continue;
            long rel = addr - base;
            long slot = rel / stride;
            long off = rel % stride;
            hits.get(name)
                .computeIfAbsent(off, k -> new TreeMap<>())
                .merge(size, 1, Integer::sum);
            slots.get(name)
                .computeIfAbsent(off, k -> new java.util.TreeSet<>())
                .add(slot);
            return;
        }
    }
}
