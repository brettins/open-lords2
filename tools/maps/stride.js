const fs=require('fs');
const f=process.argv[2]||'F:/games/Lords of the Realm II/L2_maps.dat';
const b=fs.readFileSync(f);
console.log('size',b.length);
// autocorrelation: fraction of equal bytes at lag d
function score(d,start,len){let eq=0,n=0;for(let i=start;i+d<start+len&&i+d<b.length;i++){n++;if(b[i]===b[i+d])eq++;}return eq/n;}
const res=[];
for(let d=1;d<=2048;d++)res.push([d,score(d,0,200000)]);
res.sort((a,x)=>x[1]-a[1]);
console.log('top lags (first 200KB):');
res.slice(0,25).forEach(([d,s])=>console.log('  lag',d,s.toFixed(4)));
