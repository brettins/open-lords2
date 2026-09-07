// List every reference into an address range. args: hexStart hexEndExclusive
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import java.util.*;

public class RefsRange extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        long lo = Long.parseLong(a[0], 16), hi = Long.parseLong(a[1], 16);
        ReferenceIterator it = currentProgram.getReferenceManager().getReferenceIterator(
                currentProgram.getMinAddress());
        TreeMap<String, List<String>> byFunc = new TreeMap<>();
        while (it.hasNext()) {
            Reference r = it.next();
            long to = r.getToAddress().getOffset();
            if (to < lo || to >= hi) continue;
            Function f = getFunctionContaining(r.getFromAddress());
            String k = f == null ? "(none)" : f.getEntryPoint() + " " + f.getName();
            byFunc.computeIfAbsent(k, x -> new ArrayList<>())
                  .add(r.getFromAddress() + "->" + r.getToAddress() + ":" + r.getReferenceType());
        }
        for (Map.Entry<String, List<String>> e : byFunc.entrySet())
            println(e.getKey() + "   " + e.getValue());
        println("### total funcs: " + byFunc.size());
    }
}
