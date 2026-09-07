//@category Maps
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;
import ghidra.program.model.symbol.*;
import ghidra.program.model.data.StringDataInstance;
import ghidra.program.model.mem.MemoryBlock;
import java.util.*;

public class FindMapConsts extends GhidraScript {
    @Override public void run() throws Exception {
        long[] targets = {32961L, 24576L, 8385L, 4096L, 8192L, 65L, 129L, 4225L, 2636880L, 1318440L};
        Set<Long> want = new HashSet<>();
        for (long t : targets) want.add(t);
        Map<Long, List<String>> hits = new TreeMap<>();

        InstructionIterator it = currentProgram.getListing().getInstructions(true);
        while (it.hasNext()) {
            Instruction ins = it.next();
            for (int i = 0; i < ins.getNumOperands(); i++) {
                Object[] objs = ins.getOpObjects(i);
                for (Object o : objs) {
                    if (o instanceof Scalar) {
                        long v = ((Scalar) o).getUnsignedValue();
                        if (want.contains(v)) {
                            Function f = getFunctionContaining(ins.getAddress());
                            String fn = f == null ? "?" : f.getEntryPoint() + " " + f.getName();
                            hits.computeIfAbsent(v, k -> new ArrayList<>())
                                .add(ins.getAddress() + "  " + fn + "  | " + ins.toString());
                        }
                    }
                }
            }
        }
        for (Map.Entry<Long, List<String>> e : hits.entrySet()) {
            List<String> l = e.getValue();
            println("=== constant " + e.getKey() + " (0x" + Long.toHexString(e.getKey()) + ")  hits=" + l.size());
            int n = Math.min(l.size(), 40);
            for (int i = 0; i < n; i++) println("   " + l.get(i));
            if (l.size() > n) println("   ... " + (l.size()-n) + " more");
        }

        println("\n=== strings containing 'map' / '.dat' ===");
        DataIterator di = currentProgram.getListing().getDefinedData(true);
        while (di.hasNext()) {
            Data d = di.next();
            if (d.getDataType().getName().toLowerCase().contains("string")) {
                Object val = d.getValue();
                if (val == null) continue;
                String s = val.toString();
                String ls = s.toLowerCase();
                if (ls.contains("map") || ls.contains(".dat") || ls.contains("count")
                    || ls.contains("terrain") || ls.contains("castle") || ls.contains("village")) {
                    StringBuilder refs = new StringBuilder();
                    ReferenceIterator ri = currentProgram.getReferenceManager()
                        .getReferencesTo(d.getAddress());
                    int c = 0;
                    while (ri.hasNext() && c < 8) {
                        Reference r = ri.next();
                        Function f = getFunctionContaining(r.getFromAddress());
                        refs.append(" <- ").append(r.getFromAddress())
                            .append(f == null ? "" : "(" + f.getName() + ")");
                        c++;
                    }
                    println(d.getAddress() + "  \"" + s + "\"" + refs);
                }
            }
        }
    }
}
