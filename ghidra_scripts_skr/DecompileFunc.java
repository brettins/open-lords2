// Decompile one or more functions to C. Pass addresses as script arguments.
//@category Lords2

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class DecompileFunc extends GhidraScript {

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        int maxLines = Integer.getInteger("l2.maxlines", 70);

        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        try {
            for (String a : args) {
                Address addr = toAddr(a);
                Function f = getFunctionContaining(addr);
                if (f == null) {
                    println("### no function at " + a);
                    continue;
                }
                println("### " + f.getEntryPoint() + " " + f.getName()
                        + "  params=" + f.getParameterCount());
                DecompileResults res = di.decompileFunction(f, 60, monitor);
                if (!res.decompileCompleted()) {
                    println("### decompile failed: " + res.getErrorMessage());
                    continue;
                }
                String[] lines = res.getDecompiledFunction().getC().split("\n");
                for (int i = 0; i < Math.min(lines.length, maxLines); i++) {
                    println("| " + lines[i]);
                }
                if (lines.length > maxLines) {
                    println("| ... (" + (lines.length - maxLines) + " more lines)");
                }
            }
        } finally {
            di.dispose();
        }
    }
}
