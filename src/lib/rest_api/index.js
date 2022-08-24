function Enum(obj) {
    const newObj = {};
    for( const prop in obj ) {
        if (obj.hasOwnProperty(prop)) {
            newObj[prop] = Symbol(obj[prop]);
        }
    }
    return Object.freeze(newObj);
}
const States = Enum({ AddingPolygon: true, Waiting: true });
var currentState = States.Waiting;

const drawCountour = (fbCanvas, coordinates) => {
    const clientW = fbCanvas.clientWidth;
    const clientH = fbCanvas.clientHeight;
    let ctx = fbCanvas.getContext('2d');
    ctx.lineWidth = 4;
    ctx.strokeStyle = 'blue';
    ctx.clearRect(0, 0, clientW, clientH);
    ctx.beginPath();
    ctx.moveTo(coordinates[0].x, coordinates[0].y);
    for(index=1; index<coordinates.length;index++) {
        ctx.lineTo(coordinates[index].x, coordinates[index].y);
    }
    ctx.closePath();
    ctx.stroke();
}

const minMax = (arr) => {
    return arr.reduce(function(acc, cur) {
        console.log(acc, cur)
        return [
            Math.min(cur.x, acc[0].x),
            Math.max(cur.x, acc[1].x)
        ]
    }, [{x: Number.POSITIVE_INFINITY}, {x: Number.NEGATIVE_INFINITY}]);
}

const findLeftTopY = (coordinates) => {
    return Math.abs(Math.min.apply(Math, coordinates.map(function(a) { 
        return a.y;
    })));

}

const findLefTopX = (coordinates) => {
    return Math.abs(Math.min.apply(Math, coordinates.map(function(a) { 
        return a.x;
    })));
}

const makeContour = (coordinates) => {
    let left = findLefTopX(coordinates);
    let top = findLeftTopY(coordinates);
    coordinates[coordinates.length-1] = coordinates[0];                  
    let contour = new fabric.Polyline(coordinates, {
        fill: 'rgba(0,0,0,0)',
        stroke:'#58c',
        strokeWidth: 3
    });
    contour.set({
        left: left,
        top: top,
    });
    return contour;
}

const getClickPoint = (fbCanvas, options) => {
    // const bbox = canvas.getBoundingClientRect();
    // const left = bbox.left;
    // const top = bbox.top;
    const left = fbCanvas._offset.left;
    const top = fbCanvas._offset.top;
    const drawX = options.e.pageX - left;
    const drawY = options.e.pageY - top;
    return {x: drawX, y: drawY};
}


window.onload = function() {
    let map = new maplibregl.Map({
        container: 'map', // container id
        style: 'https://demotiles.maplibre.org/style.json', // style URL
        center: [0, 0], // starting position [lng, lat]
        zoom: 1 // starting zoom
    });
    let addBtn = document.getElementById('add-btn');
    addBtn.addEventListener('click', (e) => {
        if (currentState !== States.AddingPolygon) {
            currentState = States.AddingPolygon
        }
    });

    let canvas = document.getElementById('fit_canvas');
    let image = document.getElementById('fit_img');
    canvas.width = image.clientWidth;
    canvas.height = image.clientHeight;
    let fbCanvas = new fabric.Canvas('fit_canvas', {containerClass: 'custom-container-canvas'});


    let contourTemporary = [];
    let contourFinalized = [];
    fbCanvas.on('mouse:down', (options) => {
        if (currentState === States.AddingPolygon) {
            fbCanvas.selection = false;
            let clicked = getClickPoint(fbCanvas, options);
            contourFinalized.push({ x: clicked.x, y: clicked.y });
            let points = [clicked.x, clicked.y, clicked.x, clicked.y]
            let newLine = new fabric.Line(points, {
                strokeWidth: 3,
                selectable: false,
                stroke: 'purple',
            })
            // contourTemporary.push(n.setOriginX(clickX).setOriginY(clickY));
            contourTemporary.push(newLine);
            fbCanvas.add(newLine);
            fbCanvas.on('mouse:up', function (options) {
                fbCanvas.selection = true;
            });
        }
    });

    fbCanvas.on('mouse:move', (options) => {
        if (contourTemporary[0] !== null && contourTemporary[0] !== undefined && currentState === States.AddingPolygon) {
            let clicked = getClickPoint(fbCanvas, options);
            contourTemporary[contourTemporary.length - 1].set({ x2: clicked.x, y2: clicked.y });
            fbCanvas.renderAll();
        }
    });

    fbCanvas.on('mouse:dblclick', (options) => {
        contourTemporary.forEach((value) => {
            fbCanvas.remove(value);
        });
        let contour = makeContour(contourFinalized);
        fbCanvas.add(contour);
        fbCanvas.renderAll();
        contourTemporary = [];
        contourFinalized = [];
        currentState = States.Waiting;
    });
}