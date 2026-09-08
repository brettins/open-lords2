// Dump a table of 32-bit pointers, resolving each to a function name where one exists.
// args: outFile hexAddr count [hexAddr count ...]
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import java.io.*;

public class BTable extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        for (int i = 1; i + 1 < a.length; i += 2) {
            Address base = toAddr(Long.parseLong(a[i], 16));
            int n = Integer.parseInt(a[i + 1]);
            out.println("### table " + base + " x" + n);
            for (int j = 0; j < n; j++) {
                Address slot = base.add(j * 4);
                long v = currentProgram.getMemory().getInt(slot) & 0xFFFFFFFFL;
                String nm = "-";
                if (v != 0) {
                    Address t = toAddr(v);
                    Function f = getFunctionAt(t);
                    if (f == null) f = getFunctionContaining(t);
                    if (f != null) nm = f.getName() + (f.getEntryPoint().getOffset() == v ? "" : " +off");
                }
                out.println(String.format("  [%2d] %s -> %08x  %s", j, slot, v, nm));
            }
        }
        out.close();
        println("### wrote " + a[0]);
    }
}
