const UUIDv4 = new function() {
    // https://dirask.com/posts/JavaScript-UUID-function-in-Vanilla-JS-1X9kgD
	const generateNumber = (limit) => {
	   const value = limit * Math.random();
	   return value | 0;
	};
	const generateX = () => {
		const value = generateNumber(16);
		return value.toString(16);
	};
	const generateXes = (count) => {
		let result = '';
		for(let i = 0; i < count; ++i) {
			result += generateX();
		}
		return result;
	};
	const generateVariant = () => {
		const value = generateNumber(16);
		const variant = (value & 0x3) | 0x8;
		return variant.toString(16);
	};
    // UUID v4
    //
    //   varsion: M=4 
    //   variant: N
    //   pattern: xxxxxxxx-xxxx-Mxxx-Nxxx-xxxxxxxxxxxx
    //
	this.generate = function() {
  	    const result = generateXes(8)
  	           + '-' + generateXes(4)
  	           + '-' + '4' + generateXes(3)
  	           + '-' + generateVariant() + generateXes(3)
  	           + '-' + generateXes(12)
  	    return result;
	};
};

function getRandomRGB() {
    // https://stackoverflow.com/a/23095731/6026885
    const num = Math.round(0xffffff * Math.random());
    const r = num >> 16;
    const g = num >> 8 & 255;
    const b = num & 255;
    return 'rgb(' + r + ', ' + g + ', ' + b + ')';
}

const rgba2array = (rgbValue) => {
    // https://stackoverflow.com/a/34980657/6026885
    const match = rgbValue.match(/rgba?\((\d{1,3}), ?(\d{1,3}), ?(\d{1,3})\)?(?:, ?(\d(?:\.\d?))\))?/);
    return match ? [
        match[1],
        match[2],
        match[3]
    ].map(Number) : [];
}

function Enum(obj) {
    const newObj = {};
    for( const prop in obj ) {
        if (obj.hasOwnProperty(prop)) {
            newObj[prop] = Symbol(obj[prop]);
        }
    }
    return Object.freeze(newObj);
}
const States = Enum({ AddingPolygon: true, Waiting: true, EditingPolygon: true, DeletingPolygon: true, PickPolygon: true});

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

const makeContour = (coordinates, color = getRandomRGB()) => {
    let left = findLefTopX(coordinates);
    let top = findLeftTopY(coordinates);
    // coordinates[coordinates.length-1] = coordinates[0];  // In case of fabric.Polyline               
    let contour = new fabric.Polygon(coordinates, {
        fill: 'rgba(0,0,0,0)',
        stroke: color,
        strokeWidth: 3,
        objectCaching: false
    });
    contour.set({
        left: left,
        top: top,
    });
    return contour;
}

function editContour(contour, fbCanvas) {
    fbCanvas.setActiveObject(contour);
    contour.edit = !contour.edit;
    if (contour.edit) {
        let lastControl = contour.points.length - 1;
        contour.cornerStyle = 'circle';
        contour.cornerSize = 15;
        contour.cornerColor = 'rgba(0, 0, 255, 1.0)';
        contour.controls = contour.points.reduce(function(acc, point, index) {
            acc['p' + index] = new fabric.Control({
                positionHandler: polygonPositionHandler,
                actionHandler: anchorWrapper(index > 0 ? index - 1 : lastControl, actionHandler),
                actionName: 'modifyPolygon',
                pointIndex: index
            });
            return acc;
        }, { });
    } else {
        contour.cornerColor = 'rgb(178, 204, 255)';
        contour.cornerStyle = 'rect';
        contour.controls = fabric.Object.prototype.controls;
    }
    contour.hasBorders = !contour.edit;
    fbCanvas.requestRenderAll();
}

function deleteContour(contour, fbCanvas, dataStorage, map) {
    fbCanvas.remove(contour[0]);
    dataStorage.delete(contour[0].unid);
    if (map.getLayer(`layer-polygon-${contour[0].unid}`)) {
        map.removeLayer(`layer-polygon-${contour[0].unid}`);
    }
    if (map.getSource(`source-polygon-${contour[0].unid}`)) {
        map.removeSource(`source-polygon-${contour[0].unid}`);
    }
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

async function getPolygons() {
    return await axios({
        method: 'GET',
        url: 'http://localhost:42001/api/polygons/geojson',
        timeout: 5000,
        headers: {
            'Content-Type': 'application/json'
        }
    })
    .then(res => res.data)
    .catch (err => console.error(err));
}

const drawGeoPolygons = (map, draw, dataStorage) => {
    dataStorage.forEach(feature => {
        draw.add(feature);
    });
    if (dataStorage.length === 0) {
        return
    }
    const firstCoordinates = Array.from(dataStorage.values())[0].geometry.coordinates;
    let llBbox = new maplibregl.LngLatBounds(firstCoordinates[0]);
    for (const coord of firstCoordinates) {
        llBbox.extend(coord);
    }
    map.fitBounds(llBbox, {
        padding: 20
    });
};

const drawCanvasPolygons = (app) => {
    let fbCanvas = app.fbCanvas;
    let dataStorage = app.dataStorage;
    let state = app.state;
    let scaleWidth = app.scaleWidth;
    let scaleHeight = app.scaleHeight;
    dataStorage.forEach(feature => {
        const contourFinalized = feature.properties.coordinates.map(element => {
            return {
                x: element[0]*scaleWidth,
                y: element[1]*scaleHeight
            }
        });
        let contour = makeContour(contourFinalized, `rgb(${feature.properties.color_rgb[0]},${feature.properties.color_rgb[1]},${feature.properties.color_rgb[2]})`);
        contour.on('mousedown', (options) => {
            options.e.preventDefault();
            options.e.stopPropagation();
            state = States.PickPolygon;
            if (options.button === 3) {
                state = States.EditingPolygon;
                if (state !== States.EditingPolygon) {
                    state = States.EditingPolygon;
                } else {
                    state = States.Waiting;
                }
                editContour(contour, fbCanvas);
            }
        });
        contour.unid = feature.id;
        fbCanvas.add(contour);
        fbCanvas.renderAll();
    })
}

class ApplicationUI {
    constructor() {
        this.dataStorage = new Map();
        this.initCanvas();
        this.state = States.Waiting;
        this.contourTemporary = [];
        this.contourFinalized = [];
    }
    initCanvas() {
        let canvas = document.getElementById('fit_canvas');
        let image = document.getElementById('fit_img');
        canvas.width = image.clientWidth;
        canvas.height = image.clientHeight;
        let scaleWidth = image.clientWidth/image.naturalWidth;
        this.scaleWidth = scaleWidth;
        let scaleHeight = image.clientHeight/image.naturalHeight;
        this.scaleHeight = scaleHeight;
        this.fbCanvas = new fabric.Canvas('fit_canvas', {
            containerClass: 'custom-container-canvas',
            fireRightClick: true,  
            fireMiddleClick: true, 
            stopContextMenu: true
        });
        this.fbCanvasParent = document.getElementsByClassName('custom-container-canvas')[0];
        this.fbCanvasParent.id = "fbcanvas";
        this.fbCanvas.on('selection:created', (options) => {
            if (this.state === States.DeletingPolygon) {
                this.deletePolygon(options.selected[0].unid);
                this.state = States.Waiting;
            }
        })
        this.fbCanvas.on('selection:updated', (options) => {
            if (this.state === States.DeletingPolygon) {
                this.deletePolygon(options.selected[0].unid);
                this.state = States.Waiting;
            }
        })
        this.fbCanvas.on('mouse:move', (options) => {
            if (this.contourTemporary[0] !== null && this.contourTemporary[0] !== undefined && this.state === States.AddingPolygon) {
                let clicked = getClickPoint(this.fbCanvas, options);
                this.contourTemporary[this.contourTemporary.length - 1].set({ x2: clicked.x, y2: clicked.y });
                this.fbCanvas.renderAll();
            }
        });
        this.fbCanvas.on('mouse:down', (options) => {
            if (this.state === States.AddingPolygon) {
                this.fbCanvas.selection = false;
                let clicked = getClickPoint(this.fbCanvas, options);
                this.contourFinalized.push({ x: clicked.x, y: clicked.y });
                let points = [clicked.x, clicked.y, clicked.x, clicked.y]
                let newLine = new fabric.Line(points, {
                    strokeWidth: 3,
                    selectable: false,
                    stroke: 'purple',
                })
                // this.contourTemporary.push(n.setOriginX(clickX).setOriginY(clickY));
                this.contourTemporary.push(newLine);
                this.fbCanvas.add(newLine);
                this.fbCanvas.on('mouse:up', function (options) {
                    this.selection = true;
                });
        
                if (this.contourFinalized.length > 3) {
                    this.contourTemporary.forEach((value) => {
                        this.fbCanvas.remove(value);
                    });
                    let contour = makeContour(this.contourFinalized);
                    contour.on('mousedown', (options) => {
                        options.e.preventDefault();
                        options.e.stopPropagation();
                        this.state = States.PickPolygon;
                        // Handle right-click
                        // Turn on "Edit" mode
                        if (options.button === 3) {
                            // addTooltip(fbCanvasParent, options);
                            if (this.state !== States.EditingPolygon) {
                                this.state = States.EditingPolygon;
                            } else {
                                this.state = States.Waiting;
                            }
                            editContour(contour, this.fbCanvas);
                        }
                    });
                    contour.unid = UUIDv4.generate();
                    this.dataStorage.set(contour.unid, {
                        type: 'Feature',
                        id: contour.unid,
                        properties: {
                            'color_rgb': rgba2array(contour.stroke),
                            'color_rgb_str': contour.stroke,
                            'coordinates': contour.points.map(element => {
                                return [
                                    Math.floor(element.x/scaleWidth),
                                    Math.floor(element.y/scaleHeight)
                                ]
                            }),
                            'road_lane_direction': -1,
                            'road_lane_num': -1,
                            'spatial_object_id': null,
                            'canvas_object_id': null,
                        },
                        geometry: {
                            type: 'Polygon',
                            coordinates: [[[], [], [], [], []]]
                        }
                    });
                    this.fbCanvas.add(contour);
                    this.fbCanvas.renderAll();
                    this.contourTemporary = [];
                    this.contourFinalized = [];
                    this.state = States.Waiting;
                    this.draw.changeMode('simple_select');
                }
            }
        });
    }
    attachMap(map) {
        this.map = map;
    }
    attachGLDraw(draw) {
        this.draw = draw;
    }
    deletePolygon(polygonID) {
        this.fbCanvas.getObjects().forEach( contour => {
            if (contour.unid === polygonID) {
                this.fbCanvas.remove(contour);
                return
            }
        })
        this.dataStorage.delete(polygonID);
        this.draw.delete(polygonID)
        // if (this.map.getLayer(`layer-polygon-${polygonID}`)) {
        //     this.map.removeLayer(`layer-polygon-${polygonID}`);
        // }
        // if (this.map.getSource(`source-polygon-${polygonID}`)) {
        //     this.map.removeSource(`source-polygon-${polygonID}`);
        // }
    }
    stateAdd() {
        if (this.state !== States.AddingPolygon) {
            this.state = States.AddingPolygon
            this.draw.changeMode('draw_restricted_polygon');
        } else {
            this.state = States.Waiting;
            this.draw.changeMode('simple_select');
        }
    }
    stateDel() {
        if (this.state !== States.DeletingPolygon) {
            this.state = States.DeletingPolygon
        } else {
            this.state = States.Waiting;
        }
        // this.draw.trash();
    }
    attachCanvasToSpatial(spatialID, canvasID, options = {road_lane_direction: -1, road_lane_num: -1, coordinates: []}) {
        if (spatialID === '' || canvasID === '' || spatialID === null || canvasID === null || spatialID === undefined|| canvasID === undefined) {
            return
        }
        let feature = this.dataStorage.get(canvasID);
        let mapFeature = this.draw.get(spatialID);

        // Reset information for previously attached DATASTORAGE object
        if (mapFeature.properties.canvas_object_id !== null && mapFeature.properties.canvas_object_id !== undefined) {
            let previousFeature = this.dataStorage.get(mapFeature.properties.canvas_object_id);
            previousFeature.properties.spatial_object_id = null;
            previousFeature.properties.road_lane_direction = -1;
            previousFeature.properties.road_lane_num = -1;
            previousFeature.geometry.coordinates = [[], [], [], [], []];
            this.dataStorage.set(mapFeature.properties.canvas_object_id, previousFeature);
        }

        // Scan for other spatial objects to share same canvas ID
        this.draw.getAll().features.forEach(element => {
            if (element.id === spatialID) {
                // Skip picked map feature
                return
            }
            if (element.properties.canvas_object_id === canvasID) {
                // Reset information for MAP object:
                element.properties.canvas_object_id = null;
                element.properties.color_rgb = [127, 127, 127];
                element.properties.color_rgb_str = EMPTY_POLYGON_RGB;
                this.draw.add(element);
                this.draw.setFeatureProperty(element.id, 'color_rgb_str', EMPTY_POLYGON_RGB);
            }
        })
        // Update information for MAP object
        mapFeature.properties.canvas_object_id = canvasID;
        mapFeature.properties.color_rgb = feature.properties.color_rgb;
        mapFeature.properties.color_rgb_str = feature.properties.color_rgb_str;
        mapFeature.properties.road_lane_direction = options.road_lane_direction;
        mapFeature.properties.road_lane_num = options.road_lane_num;
        this.draw.add(mapFeature);
        this.draw.setFeatureProperty(spatialID, 'color_rgb_str', feature.properties.color_rgb_str);
        // Update information for DATASTORE object
        feature.properties.spatial_object_id = spatialID;
        feature.properties.road_lane_direction = options.road_lane_direction;
        feature.properties.road_lane_num = options.road_lane_num;
        feature.geometry.coordinates = options.coordinates;
        this.dataStorage.set(canvasID, feature);
    }

    templateCollapsible (data) {
        let liValues = [];
        data.forEach(element => {
            const li = `
            <li>
                <div class="collapsible-header"><i class="material-icons">place</i>Polygon identifier: ${element.id}</div>
                <div class="collapsible-body">
                    <table class="collapsible-table">
                        <thead>
                            <tr>
                                <th>Attirubute</th>
                                <th>Value</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td>Road lane direction</td>
                                <td>${element.properties.road_lane_direction}</td>
                            </tr>
                            <tr>
                                <td>Road lane number</td>
                                <td>${element.properties.road_lane_num}</td>
                            </tr>
                            <tr>
                                <td>Color</td>
                                <td><div style="background-color: ${element.properties.color_rgb_str}; width: 32px; height: 16px; border: 1px solid #000000;"></div></td>
                            </tr>
                            <tr>
                                <td>Canvas coordinates</td>
                                <td>${JSON.stringify(element.properties.coordinates)}</td>
                            </tr>
                            <tr>
                                <td>Spatial coordinates</td>
                                <td>${JSON.stringify(element.geometry.coordinates)}</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </li>        
            `;
            liValues.push(li);
        });
        let html = liValues.join('\n');
        return html
    }
    
    updateCollapsible () {
        const collapsibleElem = document.getElementById('collapsible-data');
        collapsibleElem.innerHTML = this.templateCollapsible(this.dataStorage);
        const collapsibleInstances = M.Collapsible.init(collapsibleElem, {});
    }
}

window.onload = function() {
    const fixedButtons = document.querySelectorAll('.fixed-action-btn');
    const fixedButtonsInstances = M.FloatingActionButton.init(fixedButtons, {
        direction: 'left',
        hoverEnabled: false
    });

    let map = new maplibregl.Map({
        container: 'map', // container id
        style: 'https://api.maptiler.com/maps/44abc03b-626b-41bb-8fcd-a0e5083c9d0d/style.json?key=dznzK4GQ1Lj5U7XsI22j',
        center: [0, 0], // starting position [lng, lat]
        zoom: 1 // starting zoom
    });

    let app = new ApplicationUI();
    app.attachMap(map);

    PolygonFourPointsOnly.maxVertices = 4;
    let draw = new MapboxDraw({
        userProperties: true,
        displayControlsDefault: false,
        controls: {
            polygon: false,
            trash: false
        },
        modes: Object.assign({
            draw_restricted_polygon: PolygonFourPointsOnly,
        }, MapboxDraw.modes),
        styles: CUSTOM_GL_DRAW_STYLES
    });
    app.map.addControl(draw);
    app.attachGLDraw(draw);
    app.map.on("draw.create", function(e) {
        e.features[0].properties = {
            'color_rgb': [127, 127, 127],
            'color_rgb_str': EMPTY_POLYGON_RGB,
            'coordinates': e.features[0].geometry.coordinates,
            'road_lane_direction': -1,
            'road_lane_num': -1,
            'spatial_object_id': e.features[0].id,
            'canvas_object_id': null,
        }
        app.draw.add(e.features[0])
        app.state = States.Waiting;
    })
    const addBtn = document.getElementById('add-btn');
    addBtn.addEventListener('click', (e) => {
        app.stateAdd();
    });

    const delBtn = document.getElementById('del-btn');
    delBtn.addEventListener('click', (e) => {
        app.stateDel();
    });

    const collapsibleElem = document.getElementById('collapsible-data');
    // collapsibleElem.innerHTML = templateCollapsible();
    const collapsibleInstances = M.Collapsible.init(collapsibleElem, {});

    app.map.on('click', 'gl-draw-polygon-fill-inactive.cold', function (e) {
        const options = Array.from(app.dataStorage.values()).map((feature, idx) => { return `<option value="${feature.id}">${feature.id}</option>`});
        const mapFeature = app.draw.get(e.features[0].properties.id);
        const popupContent = `
<div id="custom-popup">
    <div class="row">
        <div class="input-field col s12">
            <select id="select-canvas">
                <option value="" disabled selected>Pick up polygon</option>
                ${options.join('\n')}
            </select>
            <label>Attach canvas polygons</label>
        </div>
    </div>
    <div class="row">
        <div class="input-field col s12">
            <input value="${mapFeature.properties.road_lane_direction}" id="lane-direction" type="number" class="validate">
            <label class="active" for="lane-direction">Direction value</label>
        </div>
    </div>
    <div class="row">
        <div class="input-field col s12">
            <input value="${mapFeature.properties.road_lane_num}" id="lane-number" type="number" class="validate">
            <label class="active" for="lane-number">Lane</label>
        </div>
    </div>
    <div class="row">
        <div class="col s12">
            <button id="attach-canvas-btn" class="btn-small waves-effect waves-light" type="submit" name="action" onclick>Save
                <i class="material-icons right">save</i>
            </button>
        </div>
    </div>
</div>
        `
        new maplibregl.Popup({ className: "custom-popup" })
            .setLngLat(e.lngLat)
            .setHTML(popupContent)
            .addTo(app.map);
        
        const feature = e.features[0];
        // const selects = document.querySelectorAll('select');
        const selectElem = document.getElementById("select-canvas");
        Array.from(app.dataStorage.values()).some(element => {
            // Pick default value if it's possible
            if (element.properties.spatial_object_id === mapFeature.id) {
                selectElem.value = element.id;
                return true;
            }
        })
        const selectsInstances = M.FormSelect.init(selectElem, {});

        const attachBtn = document.getElementById('attach-canvas-btn');
        attachBtn.addEventListener('click', (clickEvent) => {
            const directionElem = document.getElementById("lane-direction");
            const laneElem = document.getElementById("lane-number");
            // https://github.com/Dogfalo/materialize/issues/6536 - There is a workaround to get correct selected values via `getSelectedValues()` call
            // So just leave next two code lines just for history:
            // const selectInstance = M.FormSelect.getInstance(selectElem);
            // console.log("bug", selectInstance.getSelectedValues())
            app.attachCanvasToSpatial(feature.properties.id, selectElem.value, {road_lane_direction: directionElem.value, road_lane_num: laneElem.value, coordinates: app.draw.get(feature.properties.id).geometry.coordinates});
        });
    });

    getPolygons().then((data) => {
        data.features.forEach(feature => {
            feature.properties.spatial_object_id = feature.id;
            feature.properties.canvas_object_id = feature.id;
            feature.properties.color_rgb_str = `rgb(${feature.properties.color_rgb[0]},${feature.properties.color_rgb[1]},${feature.properties.color_rgb[2]})`;
            app.dataStorage.set(feature.id, feature);
        });
        app.map.on('load', () => {
            drawGeoPolygons(app.map, draw, app.dataStorage);
        });
        drawCanvasPolygons(app);
        app.updateCollapsible();
    })
}
