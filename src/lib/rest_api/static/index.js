function Enum(obj) {
    const newObj = {};
    for( const prop in obj ) {
        if (obj.hasOwnProperty(prop)) {
            newObj[prop] = Symbol(obj[prop]);
        }
    }
    return Object.freeze(newObj);
}
const States = Enum({ AddingPolygon: true, Waiting: true, EditingPolygon: true });
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
    // coordinates[coordinates.length-1] = coordinates[0];  // In case of fabric.Polyline               
    let contour = new fabric.Polygon(coordinates, {
        fill: 'rgba(0,0,0,0)',
        stroke:'#58c',
        strokeWidth: 3,
        objectCaching: false
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

const getObjectSizeWithStroke = (object) => {
    let stroke = new fabric.Point(
        object.strokeUniform ? 1 / object.scaleX : 1, 
        object.strokeUniform ? 1 / object.scaleY : 1
    ).multiply(object.strokeWidth);
    return new fabric.Point(object.width + stroke.x, object.height + stroke.y);
}

// define a function that can locate the controls.
// this function will be used both for drawing and for interaction.
// this is not an anonymus function since we need parent scope (`this`)
const polygonPositionHandler = function (dim, finalMatrix, fabricObject) {
    let x = (fabricObject.points[this.pointIndex].x - fabricObject.pathOffset.x);
    let y = (fabricObject.points[this.pointIndex].y - fabricObject.pathOffset.y);
    return fabric.util.transformPoint(
        { x: x, y: y },
        fabric.util.multiplyTransformMatrices(
            fabricObject.canvas.viewportTransform,
            fabricObject.calcTransformMatrix()
        )
    );
}

// define a function that will define what the control does
// this function will be called on every mouse move after a control has been
// clicked and is being dragged.
// The function receive as argument the mouse event, the current trasnform object
// and the current position in canvas coordinate
// transform.target is a reference to the current object being transformed,
const actionHandler = function (eventData, transform, x, y) {
    let polygon = transform.target;
    let currentControl = polygon.controls[polygon.__corner];
    let mouseLocalPosition = polygon.toLocalPoint(new fabric.Point(x, y), 'center', 'center')
    let polygonBaseSize = getObjectSizeWithStroke(polygon);
    let size = polygon._getTransformedDimensions(0, 0);
    let finalPointPosition = {
        x: mouseLocalPosition.x * polygonBaseSize.x / size.x + polygon.pathOffset.x,
        y: mouseLocalPosition.y * polygonBaseSize.y / size.y + polygon.pathOffset.y
    };
    polygon.points[currentControl.pointIndex] = finalPointPosition;
    return true;
}

// define a function that can keep the polygon in the same position when we change its
// width/height/top/left.
const anchorWrapper = function (anchorIndex, fn) {
    return function(eventData, transform, x, y) {
        let fabricObject = transform.target;
        let absolutePoint = fabric.util.transformPoint({
            x: (fabricObject.points[anchorIndex].x - fabricObject.pathOffset.x),
            y: (fabricObject.points[anchorIndex].y - fabricObject.pathOffset.y),
        }, fabricObject.calcTransformMatrix());
        let actionPerformed = fn(eventData, transform, x, y);
        let newDim = fabricObject._setPositionDimensions({});
        let polygonBaseSize = getObjectSizeWithStroke(fabricObject);
        let newX = (fabricObject.points[anchorIndex].x - fabricObject.pathOffset.x) / polygonBaseSize.x;
        let newY = (fabricObject.points[anchorIndex].y - fabricObject.pathOffset.y) / polygonBaseSize.y;
        fabricObject.setPositionByOrigin(absolutePoint, newX + 0.5, newY + 0.5);
        return actionPerformed;
    }
}

const addTooltip = (parentDiv, options) => {
    // @todo make cool popup with edit/trash signs

    // let div = document.createElement('div');
    // div.style.cssText = 'position:fixed;padding:7px;background:gold;pointer-events:none;width:30px';
    // div.innerHTML = 'potato';
    // div.style.left = `${options.target.aCoords.br.x}px`;
    // div.style.top = `${options.target.aCoords.br.y}px`; 
    // parentDiv.appendChild(div);
}

window.onload = function() {
    let map = new maplibregl.Map({
        container: 'map', // container id
        style: 'https://api.maptiler.com/maps/44abc03b-626b-41bb-8fcd-a0e5083c9d0d/style.json?key=dznzK4GQ1Lj5U7XsI22j',
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
    let fbCanvas = new fabric.Canvas('fit_canvas', {
        containerClass: 'custom-container-canvas',
        fireRightClick: true,  
        fireMiddleClick: true, 
        stopContextMenu: true
    });
    let fbCanvasParent = document.getElementsByClassName('custom-container-canvas')[0];
    fbCanvasParent.id = "fbcanvas";

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

            if (contourFinalized.length > 3) {
                contourTemporary.forEach((value) => {
                    fbCanvas.remove(value);
                });
                let contour = makeContour(contourFinalized);
                contour.on('mousedown', (options) => {
                    options.e.preventDefault();
                    options.e.stopPropagation();
                    // Handle right-click
                    // Turn on "Edit" mode
                    if (options.button === 3) {
                        // addTooltip(fbCanvasParent, options);
                        currentState = States.EditingPolygon;
                        editContour(contour, fbCanvas);
                    }
                });
                fbCanvas.add(contour);
                fbCanvas.renderAll();
                contourTemporary = [];
                contourFinalized = [];
                currentState = States.Waiting;
            }
        }
    });

    fbCanvas.on('mouse:move', (options) => {
        if (contourTemporary[0] !== null && contourTemporary[0] !== undefined && currentState === States.AddingPolygon) {
            let clicked = getClickPoint(fbCanvas, options);
            contourTemporary[contourTemporary.length - 1].set({ x2: clicked.x, y2: clicked.y });
            fbCanvas.renderAll();
        }
    });

    function editContour(editContour, fbCanvas) {
		fbCanvas.setActiveObject(editContour);
        editContour.edit = !editContour.edit;
        if (editContour.edit) {
            let lastControl = editContour.points.length - 1;
            editContour.cornerStyle = 'circle';
            editContour.cornerSize = 15;
            editContour.cornerColor = 'rgba(0, 0, 255, 1.0)';
            editContour.controls = editContour.points.reduce(function(acc, point, index) {
				acc['p' + index] = new fabric.Control({
					positionHandler: polygonPositionHandler,
					actionHandler: anchorWrapper(index > 0 ? index - 1 : lastControl, actionHandler),
					actionName: 'modifyPolygon',
					pointIndex: index
				});
				return acc;
			}, { });
        } else {
            editContour.cornerColor = 'rgb(178, 204, 255)';
            editContour.cornerStyle = 'rect';
			editContour.controls = fabric.Object.prototype.controls;
        }
        editContour.hasBorders = !editContour.edit;
		fbCanvas.requestRenderAll();
    }
}