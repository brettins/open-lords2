// Decompile a function, printing a line range. args: hexAddr firstLine lastLine
//@category Lords2
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;

public class DecompSlice extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        int from = a.length > 1 ? Integer.parseInt(a[1]) : 0;
        int to   = a.length > 2 ? Integer.parseInt(a[2]) : 100000;
        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        try {
            Function f = getFunctionContaining(toAddr(a[0]));
            DecompileResults res = di.decompileFunction(f, 120, monitor);
            String[] L = res.getDecompiledFunction().getC().split("\n");
            println("### " + f.getEntryPoint() + " total=" + L.length);
            for (int i = from; i < Math.min(L.length, to); i++) println("|" + i + "| " + L[i]);
        } finally { di.dispose(); }
    }
}
