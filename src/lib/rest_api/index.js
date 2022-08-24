window.onload = function() {
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

    var map = new maplibregl.Map({
        container: 'map', // container id
        style: 'https://demotiles.maplibre.org/style.json', // style URL
        center: [0, 0], // starting position [lng, lat]
        zoom: 1 // starting zoom
    });
    var state = {

    }
    let polygon = [];
    let isDone = false;
    function drawCountour(canvas, coordinates){
        const clientW = canvas.clientWidth;
        const clientH = canvas.clientHeight;
        canvas.width = clientW;
        canvas.height = clientH;
        let ctx = canvas.getContext('2d');
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

    let canvas = document.getElementById('fit_canvas');
    let image = document.getElementById('fit_img');

    // FabricJS stuff
    let drawX = 0;
    let drawY = 0;

    function setStartingPoint(options) {
        const bbox = canvas.getBoundingClientRect();
        const left = bbox.left;
        const top = bbox.top;
        drawX = options.e.pageX - left;
        drawY = options.e.pageY - top;
    }
    function findTopPaddingForRoof(coordinates) {
        var result = 999999;
        for (var f = 0; f < lineCounter; f++) {
            if (coordinates[f].y < result) {
                result = coordinates[f].y;
            }
        }
        return Math.abs(result);
    }
    function findLeftPaddingForRoof(coordinates) {
        var result = 999999;
        for (var i = 0; i < lineCounter; i++) {
            if (coordinates[i].x < result) {
                result = coordinates[i].x;
            }
        }
        return Math.abs(result);
    }
    function makeContour(coordinates) {
        let left = findLeftPaddingForRoof(coordinates);
        let top = findTopPaddingForRoof(coordinates);
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

    
    const clientW = image.clientWidth;
    const clientH = image.clientHeight;
    canvas.width = clientW;
    canvas.height = clientH;
    let fbCanvas = new fabric.Canvas('fit_canvas', {containerClass: 'custom-container-canvas'});
    let lines = [];
    let lineCounter = 0;
    fbCanvas.on('mouse:down', (options) => {
        if (currentState === States.AddingPolygon) {
            fbCanvas.selection = false;
            setStartingPoint(options)
            polygon.push({ x: drawX, y: drawY });
            let points = [drawX, drawY, drawX, drawY]
            let newLine = new fabric.Line(points, {
                strokeWidth: 3,
                selectable: false,
                stroke: 'purple',
            })
            // lines.push(n.setOriginX(clickX).setOriginY(clickY));
            lines.push(newLine);
            fbCanvas.add(lines[lineCounter]);
            lineCounter += 1;
            fbCanvas.on('mouse:up', function (options) {
                fbCanvas.selection = true;
            });
        }
    });

    fbCanvas.on('mouse:move', (options) => {
        if (lines[0] !== null && lines[0] !== undefined && currentState === States.AddingPolygon) {
            setStartingPoint(options);
            lines[lineCounter - 1].set({
                x2: drawX,
                y2: drawY
            });
            fbCanvas.renderAll();
        }
    });

    fbCanvas.on('mouse:dblclick', (options) => {
        lines.forEach((value, index, ar) => {
            fbCanvas.remove(value);
        });
        let contour = makeContour(polygon);
        fbCanvas.add(contour);
        fbCanvas.renderAll();
        lines = [];
        lineCounter = 0;
        polygon = [];
        currentState = States.Waiting;
    });

    let addBtn = document.getElementById('add-btn');
    addBtn.addEventListener('click', (e) => {
        if (currentState !== States.AddingPolygon) {
            currentState = States.AddingPolygon
        }
    });
}