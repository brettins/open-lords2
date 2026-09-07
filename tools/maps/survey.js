const {plane,sheet}=require('E:/dev/lords2/tools/maps/lib.js');
const p=+(process.argv[2]||0);
const recs=[...Array(24).keys(), ...Array(20).keys()].map((v,i)=> i<24? v : v+40);
sheet(recs.map(r=>({data:plane(r,p),w:64,h:64,mode:'idx'})),64,64,8,2,`E:/dev/lords2/tools/maps/out/survey_p${p}.png`,6);
console.log('records order:',recs.join(','));
