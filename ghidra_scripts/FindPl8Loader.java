// Locate the routine that loads .pl8 sprite files.
// The exe hardcodes many "*.pl8" filenames. Find the functions referencing
// those strings, then find the callee they share - that is the loader.
//@category Lords2

import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class FindPl8Loader extends GhidraScript {

    @Override
    public void run() throws Exception {
        List<Data> names = new ArrayList<>();
        DataIterator it = currentProgram.getListing().getDefinedData(true);
        while (it.hasNext()) {
            Data d = it.next();
            Object v = d.getValue();
            if (v == null) continue;
            String s = v.toString();
            if (s.length() > 4 && s.toLowerCase().endsWith(".pl8")) {
                names.add(d);
            }
        }
        println("[pl8] defined .pl8 strings: " + names.size());
        for (int i = 0; i < Math.min(5, names.size()); i++) {
            println("      " + names.get(i).getMinAddress() + "  " + names.get(i).getValue());
        }

        Map<Function, Integer> referencing = new HashMap<>();
        for (Data d : names) {
            for (Reference r : getReferencesTo(d.getMinAddress())) {
                Function f = getFunctionContaining(r.getFromAddress());
                if (f != null) referencing.merge(f, 1, Integer::sum);
            }
        }
        println("[pl8] functions referencing .pl8 names: " + referencing.size());
        referencing.entrySet().stream()
            .sorted((a, b) -> b.getValue() - a.getValue())
            .limit(8)
            .forEach(e -> println("      " + e.getKey().getEntryPoint() + " "
                + e.getKey().getName() + "  (" + e.getValue() + " refs)"));

        Map<Function, Integer> callees = new HashMap<>();
        for (Function f : referencing.keySet()) {
            for (Function c : f.getCalledFunctions(monitor)) {
                callees.merge(c, 1, Integer::sum);
            }
        }
        println("[pl8] shared callees (loader candidates):");
        callees.entrySet().stream()
            .sorted((a, b) -> b.getValue() - a.getValue())
            .limit(12)
            .forEach(e -> println("      " + e.getKey().getEntryPoint() + " "
                + e.getKey().getName() + "  <- called by " + e.getValue()
                + " of " + referencing.size()));
    }
}
