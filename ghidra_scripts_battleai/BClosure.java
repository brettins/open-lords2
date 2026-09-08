// Transitive callee closure from a set of roots, breadth-first, with depth and callers.
// args: outFile maxDepth hexAddr [hexAddr ...]
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class BClosure extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        PrintWriter out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        int maxDepth = Integer.parseInt(a[1]);
        Map<Address,Integer> depth = new LinkedHashMap<>();
        Deque<Address> q = new ArrayDeque<>();
        for (int i = 2; i < a.length; i++) {
            Function f = getFunctionContaining(toAddr(a[i]));
            if (f == null) { out.println("### no function at " + a[i]); continue; }
            if (depth.putIfAbsent(f.getEntryPoint(), 0) == null) q.add(f.getEntryPoint());
        }
        while (!q.isEmpty()) {
            Address cur = q.poll();
            int d = depth.get(cur);
            Function f = getFunctionAt(cur);
            if (f == null) continue;
            if (d >= maxDepth) continue;
            for (Function c : f.getCalledFunctions(monitor)) {
                if (depth.putIfAbsent(c.getEntryPoint(), d + 1) == null) q.add(c.getEntryPoint());
            }
        }
        List<Map.Entry<Address,Integer>> es = new ArrayList<>(depth.entrySet());
        es.sort((x,y) -> x.getKey().compareTo(y.getKey()));
        for (Map.Entry<Address,Integer> e : es) {
            Function f = getFunctionAt(e.getKey());
            out.println(String.format("%s  d%d  %-32s body=%d", e.getKey(), e.getValue(),
                f == null ? "?" : f.getName(), f == null ? 0 : f.getBody().getNumAddresses()));
        }
        out.println("### total " + depth.size());
        out.close();
        println("### wrote " + a[0] + " total " + depth.size());
    }
}
