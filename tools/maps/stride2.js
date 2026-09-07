const fs=require('fs');
const b=fs.readFileSync('F:/games/Lords of the Realm II/L2_maps.dat');
function best(start,len,maxd){let bd=0,bs=-1;for(let d=2;d<=maxd;d++){let eq=0,n=0;for(let i=start;i+d<start+len;i++){n++;if(b[i]===b[i+d])eq++;}const s=eq/n;if(s>bs){bs=s;bd=d;}}return [bd,bs];}
const W=32768;
for(let off=0;off+W<=b.length;off+=W){
  const [d,s]=best(off,W,600);
  // also distinct byte count & zero frac
  const set=new Set();let z=0;for(let i=off;i<off+W;i++){set.add(b[i]);if(b[i]===0)z++;}
  console.log(off.toString().padStart(8), '0x'+off.toString(16).padStart(6,'0'), 'lag',String(d).padStart(4), s.toFixed(3), 'distinct',String(set.size).padStart(3),'zero%',(100*z/W).toFixed(0));
}
