// List functions referencing given addresses (hex args), with ref type.
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;
import java.util.*;

public class RefsTo extends GhidraScript {
    @Override public void run() throws Exception {
        for (String a : getScriptArgs()) {
            Address addr = toAddr(Long.parseLong(a, 16));
            println("### refs to " + addr);
            Map<String,List<String>> byFunc = new TreeMap<>();
            for (Reference r : getReferencesTo(addr)) {
                Function f = getFunctionContaining(r.getFromAddress());
                String k = f == null ? "(none)" : f.getEntryPoint() + " " + f.getName();
                byFunc.computeIfAbsent(k, x -> new ArrayList<>())
                      .add(r.getFromAddress() + ":" + r.getReferenceType());
            }
            for (Map.Entry<String,List<String>> e : byFunc.entrySet())
                println("   " + e.getKey() + "  " + e.getValue());
        }
    }
}
