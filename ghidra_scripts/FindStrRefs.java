// Find memory occurrences of literal ASCII strings (args) and report referencing functions.
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.mem.Memory;

public class FindStrRefs extends GhidraScript {
    @Override public void run() throws Exception {
        Memory mem = currentProgram.getMemory();
        for (String s : getScriptArgs()) {
            println("### \"" + s + "\"");
            Address a = currentProgram.getMinAddress();
            while (a != null) {
                Address hit = mem.findBytes(a, s.getBytes("ASCII"), null, true, monitor);
                if (hit == null) break;
                println("  @ " + hit);
                for (Reference r : getReferencesTo(hit)) {
                    Function f = getFunctionContaining(r.getFromAddress());
                    println("      <- " + r.getFromAddress() + " " + r.getReferenceType()
                        + "  in " + (f==null?"(none)":f.getEntryPoint()+" "+f.getName()));
                }
                a = hit.add(1);
            }
        }
    }
}
