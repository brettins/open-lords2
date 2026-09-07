// Confirm how the PL8 blitter is selected, and inspect the mode-1 blitter.
//@category Lords2

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.RefType;

public class Pl8Modes extends GhidraScript {

    @Override
    public void run() throws Exception {
        Address sel = toAddr("005cdd20");
        println("### references to the blitter selector at " + sel);
        for (Reference r : getReferencesTo(sel)) {
            Function f = getFunctionContaining(r.getFromAddress());
            RefType t = r.getReferenceType();
            println("###   " + r.getFromAddress() + "  " + t
                    + (t.isWrite() ? "  <== WRITE" : "")
                    + "  in " + (f == null ? "<none>" : f.getName() + " @" + f.getEntryPoint()));
        }

        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        try {
            for (String a : new String[] { "004b446d" }) {
                Function f = getFunctionContaining(toAddr(a));
                println("### " + f.getEntryPoint() + " " + f.getName());
                DecompileResults res = di.decompileFunction(f, 60, monitor);
                if (!res.decompileCompleted()) { println("### failed"); continue; }
                String[] lines = res.getDecompiledFunction().getC().split("\n");
                for (int i = 0; i < Math.min(lines.length, 55); i++) println("| " + lines[i]);
                if (lines.length > 55) println("| ... " + (lines.length - 55) + " more");
            }
        } finally {
            di.dispose();
        }
    }
}
