// Decompile any number of functions in one headless run, writing full C to a file.
// args: outFile hexAddr [hexAddr ...]
//@category Lords2
import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.io.*;
import java.util.*;

public class KDecomp extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        Set<String> done = new HashSet<>();
        try {
            for (int i = 1; i < a.length; i++) {
                Address addr = toAddr(a[i]);
                Function f = getFunctionContaining(addr);
                if (f == null) { out.println("### no function at " + a[i]); continue; }
                if (!done.add(f.getEntryPoint().toString())) continue;
                out.println("### " + f.getEntryPoint() + " " + f.getName() + " params=" + f.getParameterCount());
                DecompileResults res = di.decompileFunction(f, 180, monitor);
                if (!res.decompileCompleted()) { out.println("### FAILED " + res.getErrorMessage()); continue; }
                out.println(res.getDecompiledFunction().getC());
                out.println();
            }
        } finally { di.dispose(); out.close(); }
        println("### wrote " + a[0]);
    }
}
