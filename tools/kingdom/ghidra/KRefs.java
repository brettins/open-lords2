// Cross-reference helper. args: outFile mode ...
//   mode "to"    : hexAddr ...            -> functions referencing each address
//   mode "range" : hexLo hexHiExcl        -> every reference into [lo,hi)
//   mode "callers": hexFuncAddr ...       -> callers of each function
//   mode "callees": hexFuncAddr ...       -> callees of each function
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class KRefs extends GhidraScript {
    PrintWriter out;
    String fname(Address a) { Function f = getFunctionContaining(a); return f == null ? "(none)" : f.getEntryPoint() + " " + f.getName(); }
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        out = new PrintWriter(new BufferedWriter(new FileWriter(a[0])));
        String mode = a[1];
        if (mode.equals("range")) {
            long lo = Long.parseLong(a[2],16), hi = Long.parseLong(a[3],16);
            ReferenceIterator it = currentProgram.getReferenceManager().getReferenceIterator(currentProgram.getMinAddress());
            TreeMap<String,List<String>> m = new TreeMap<>();
            while (it.hasNext()) {
                Reference r = it.next();
                long to = r.getToAddress().getOffset();
                if (to < lo || to >= hi) continue;
                m.computeIfAbsent(fname(r.getFromAddress()), x->new ArrayList<>())
                 .add(r.getFromAddress()+"->"+r.getToAddress()+":"+r.getReferenceType());
            }
            for (Map.Entry<String,List<String>> e : m.entrySet()) out.println(e.getKey()+"  "+e.getValue());
            out.println("### funcs: " + m.size());
        } else if (mode.equals("to")) {
            for (int i = 2; i < a.length; i++) {
                Address ad = toAddr(a[i]);
                out.println("### refs to " + ad);
                TreeMap<String,List<String>> m = new TreeMap<>();
                for (Reference r : getReferencesTo(ad))
                    m.computeIfAbsent(fname(r.getFromAddress()), x->new ArrayList<>())
                     .add(r.getFromAddress()+":"+r.getReferenceType());
                for (Map.Entry<String,List<String>> e : m.entrySet()) out.println("   "+e.getKey()+"  "+e.getValue());
            }
        } else if (mode.equals("callers")) {
            for (int i = 2; i < a.length; i++) {
                Function f = getFunctionContaining(toAddr(a[i]));
                out.println("### callers of " + (f==null?a[i]:f.getEntryPoint()+" "+f.getName()));
                if (f == null) continue;
                TreeSet<String> s = new TreeSet<>();
                for (Reference r : getReferencesTo(f.getEntryPoint())) s.add(fname(r.getFromAddress()) + " @" + r.getFromAddress());
                for (String x : s) out.println("   " + x);
            }
        } else if (mode.equals("callees")) {
            for (int i = 2; i < a.length; i++) {
                Function f = getFunctionContaining(toAddr(a[i]));
                out.println("### callees of " + (f==null?a[i]:f.getEntryPoint()+" "+f.getName()));
                if (f == null) continue;
                TreeSet<String> s = new TreeSet<>();
                for (Function c : f.getCalledFunctions(monitor)) s.add(c.getEntryPoint() + " " + c.getName());
                for (String x : s) out.println("   " + x);
            }
        }
        out.close();
        println("### wrote " + a[0]);
    }
}
